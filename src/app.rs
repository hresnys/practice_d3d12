
use core::{
    ffi::c_void,
    ptr::{null, NonNull}
};

use windows::{
    Win32::{
        UI::{
            WindowsAndMessaging::*,
            Input::KeyboardAndMouse::SetFocus
        }, 
        Foundation::{HINSTANCE, HWND, LRESULT, WPARAM, LPARAM, RECT, HANDLE, CloseHandle}, 
        System::{
            LibraryLoader::GetModuleHandleW, 
            Threading::{
                CreateEventW, 
                WaitForSingleObjectEx
            }, 
            WindowsProgramming::INFINITE}, 
        Graphics::{
            Gdi::{GetSysColorBrush, UpdateWindow}, 
            Direct3D::D3D_FEATURE_LEVEL_11_0
        },
        Graphics::Direct3D12::*,
        Graphics::Dxgi::{*, Common::*},
    }, 
    core::{PCWSTR, Error, Interface}
};

type HR<T> = Result<T, windows::core::Error>;
const CLASS_NAME : &str = "SampleWindowClass";

#[allow(dead_code)]
pub struct App {
    hinstance: HINSTANCE,
    hwnd: HWND,
    width: u32,
    height: u32,

    device: ID3D12Device,
    queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain3,
    color_buffer: [ID3D12Resource; Self::FRAME_COUNT],
    cmd_allocator: [ID3D12CommandAllocator; Self::FRAME_COUNT],
    cmd_list: ID3D12GraphicsCommandList,
    heap_rtv: ID3D12DescriptorHeap,
    fence: ID3D12Fence,
    fence_event: HANDLE,
    fence_counter: [u64; Self::FRAME_COUNT],
    frame_index: usize,
    handle_rtv: [D3D12_CPU_DESCRIPTOR_HANDLE; Self::FRAME_COUNT]
}

impl App {
    const FRAME_COUNT : usize = 2;
    pub fn new(width: u32, height: u32) -> Result<Self, Error> {
        unsafe {
            let h_instance = GetModuleHandleW(PCWSTR::default())?;
            let default_icon = HICON(LoadImageW(HINSTANCE::default(),PCWSTR(OIC_SAMPLE as _), IMAGE_ICON, 0, 0, LR_SHARED | LR_DEFAULTCOLOR | LR_DEFAULTSIZE)?.0);
            let wc = WNDCLASSEXW {
                cbSize: core::mem::size_of::<WNDCLASSEXW>() as _,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::wnd_proc),
                hIcon: default_icon,
                hCursor: HCURSOR(LoadImageW(HINSTANCE::default(),PCWSTR(OCR_NORMAL.0 as _), IMAGE_CURSOR, 0, 0, LR_SHARED | LR_DEFAULTCOLOR | LR_DEFAULTSIZE)?.0),
                hbrBackground: GetSysColorBrush(COLOR_BACKGROUND.0 as _),
                lpszMenuName: PCWSTR::default(),
                lpszClassName: PCWSTR(CLASS_NAME.encode_utf16().chain([0]).collect::<Vec<u16>>().as_ptr()),
                hIconSm: default_icon,
                ..Default::default()
            };

            if RegisterClassExW(&wc) == 0 {
                return Err(windows::core::Error::from_win32());
            }

            let mut rc = RECT {
                right: width as _,
                bottom: height as _,
                ..Default::default()
            };

            let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
            AdjustWindowRect(&mut rc, style, false);

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0), 
                CLASS_NAME, 
                "Sample", 
                style,
                CW_USEDEFAULT, CW_USEDEFAULT,
                rc.right - rc.left,
                rc.bottom - rc.top,
                None, 
                None, 
                h_instance, 
                null());

            if hwnd.0 == 0 {
                return Err(Error::from_win32());
            }
            
            ShowWindow(hwnd, SW_SHOWNORMAL);
            UpdateWindow(hwnd);
            SetFocus(hwnd);
            let device = Self::init_device()?.ok_or(Error::OK)?;
            let queue = Self::create_cmd_queue(&device)?;
            let swap_chain = Self::create_swapchain(&queue, width, height, hwnd)?;
            let frame_index = swap_chain.GetCurrentBackBufferIndex() as usize;
            let cmd_allocator : [ID3D12CommandAllocator; Self::FRAME_COUNT] = Self::create_cmd_allocator(&device)?;
            let cmd_list = Self::create_cmd_list(&device, &cmd_allocator[frame_index])?;
            let heap_rtv = Self::create_desc_heap(&device)?;
            
            let mut handle = heap_rtv.GetCPUDescriptorHandleForHeapStart();
            let increment_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
            let mut color_buffer : [NonNull<c_void>; Self::FRAME_COUNT] = [NonNull::dangling(); Self::FRAME_COUNT];
            let mut handle_rtv : [NonNull<c_void>; Self::FRAME_COUNT] = [NonNull::dangling(); Self::FRAME_COUNT];
            
            for i in 0..Self::FRAME_COUNT {
                let buf : ID3D12Resource = swap_chain.GetBuffer(i as _)?;
                println!("handle");
                
                let view_desc = D3D12_RENDER_TARGET_VIEW_DESC {
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
                    ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_RTV {
                            MipSlice: 0,
                            PlaneSlice: 0,
                        },
                    },
                };

                device.CreateRenderTargetView(buf.clone(), &view_desc, handle);
                color_buffer[i] = core::mem::transmute(buf);
                println!("handle");
                handle_rtv[i] = core::mem::transmute(handle);
                handle.ptr += increment_size as usize;
            }

            let color_buffer = core::mem::transmute(color_buffer);
            let handle_rtv = core::mem::transmute(handle_rtv);
            
            let mut fence_counter = [0u64; Self::FRAME_COUNT];
            let fence = device.CreateFence(fence_counter[frame_index], D3D12_FENCE_FLAG_NONE)?;
            fence_counter[frame_index] += 1;
            let fence_event = CreateEventW(null(), false, false, None)?;

            cmd_list.Close()?;
            Ok(Self {
                hinstance: h_instance,
                hwnd,
                width,
                height,
                device,
                queue,
                swap_chain,
                color_buffer,
                cmd_allocator,
                cmd_list,
                heap_rtv,
                fence,
                fence_event,
                fence_counter,
                frame_index,
                handle_rtv,
                
            })
        }
    }

    unsafe fn render(&mut self) -> HR<()> {
        use core::mem::ManuallyDrop;
        let frame_index = self.frame_index;
        
        // start recording commands 
        self.cmd_allocator[frame_index].Reset()?;
        self.cmd_list.Reset(&self.cmd_allocator[frame_index], None)?;

        // setting resourse barrier
        let barrier = D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: Some(self.color_buffer[frame_index].clone()),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_PRESENT,
                    StateAfter: D3D12_RESOURCE_STATE_RENDER_TARGET,
                })
            },
        };
        
        // resource barrier
        self.cmd_list.ResourceBarrier(&[barrier]);

        // setting render target
        self.cmd_list.OMSetRenderTargets(1, &self.handle_rtv[frame_index], false, null());

        // setting clear color
        let clear_color = [0.25f32, 0.25, 0.25, 1.0];

        self.cmd_list.ClearRenderTargetView(self.handle_rtv[frame_index], &clear_color as *const _, &[]);

        {
            //todo!();
        }

        // setting resourse barrier
        let barrier = D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: Some(self.color_buffer[frame_index].clone()),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_RENDER_TARGET,
                    StateAfter: D3D12_RESOURCE_STATE_PRESENT,
                })
            },
        };

        self.cmd_list.ResourceBarrier(&[barrier]);

        self.cmd_list.Close()?;

        self.queue.ExecuteCommandLists(&[Some(self.cmd_list.clone().into())]);

        self.present(1)?;
        Ok(())
    }

    unsafe fn present(&mut self, interval: u32) -> HR<()>{
        let frame_index = self.frame_index;
        self.swap_chain.Present(interval, 0)?;

        let current_value = self.fence_counter[frame_index];
        self.queue.Signal(self.fence.clone(), current_value)?;

        self.frame_index = self.swap_chain.GetCurrentBackBufferIndex() as usize;
        if self.fence.GetCompletedValue() < self.fence_counter[self.frame_index] {
            self.fence.SetEventOnCompletion(self.fence_counter[self.frame_index], self.fence_event)?;
            WaitForSingleObjectEx(self.fence_event,INFINITE, false);
        }

        self.fence_counter[self.frame_index] = current_value + 1;
        Ok(())
    }

    unsafe fn wait_gpu(&mut self) -> HR<()> {
        self.queue.Signal(self.fence.clone(), self.fence_counter[self.frame_index])?;
        self.fence.SetEventOnCompletion(self.fence_counter[self.frame_index], self.fence_event)?;
        WaitForSingleObjectEx(self.fence_event, INFINITE, false);
        self.fence_counter[self.frame_index] += 1;
        Ok(())
    }

    pub fn run(mut self) {
        //self.init_d3d();
        self.mainloop();
        self.term();
    }

    fn term(self) {
        self.term_wnd()
    }


    fn term_wnd(self) {
        if !self.hinstance.is_invalid() {
            unsafe {
                UnregisterClassW(CLASS_NAME, self.hinstance);
            }
        }
    }

    fn mainloop(&mut self) {
        let mut msg = MSG::default();
        
        while WM_QUIT != msg.message {
            unsafe {
                if PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    self.render().unwrap();
                }
            }
        }
    }

    // fn init_d3d(&self) -> Result<(), Error> {
    //     todo!()
    // }

    fn init_device() -> Result<Option<ID3D12Device>, Error> {
        unsafe {
            let mut device : Option<ID3D12Device> = None;
            // S_FALSEの時にresult__がNULLっぽい
            D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device)?;
            Ok(device)
        }
    }

    fn create_cmd_queue(device: &ID3D12Device) -> Result<ID3D12CommandQueue, Error> {
        let desc = D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            Priority: D3D12_COMMAND_QUEUE_PRIORITY_NORMAL.0,
            Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
            NodeMask: 0,
        };
        unsafe {
            device.CreateCommandQueue(&desc)
        }
    }

    fn create_swapchain(queue: &ID3D12CommandQueue, width: u32, height: u32, hwnd: HWND) -> HR<IDXGISwapChain3> {
        let factory : IDXGIFactory4 = unsafe { CreateDXGIFactory1()? };

        let desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc : DXGI_MODE_DESC {
                Width: width,
                Height: height,
                RefreshRate: DXGI_RATIONAL {
                    Numerator: 60,
                    Denominator: 1,
                },
                ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
                Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: Self::FRAME_COUNT as _,
            OutputWindow: hwnd,
            Windowed: true.into(),
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            Flags: DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH.0 as _,
        };


        let swap_chain = unsafe { factory.CreateSwapChain(queue, &desc)? };
        let mut swap_chain3 : Option<IDXGISwapChain3> = None;
        unsafe { swap_chain.query(&IDXGISwapChain3::IID, core::mem::transmute(&mut swap_chain3)).ok()?};
        
        if let Some(sc) = swap_chain3 {
            Ok(sc)
        } else {
            Err(Error::OK)
        }
    }

    fn create_cmd_allocator(device: &ID3D12Device) -> HR<[ID3D12CommandAllocator; Self::FRAME_COUNT]> {
        // 未初期化のIUnknownを作る手段がないので無理矢理。未定義動作でダメそう？
        unsafe {
            let mut allocators : [NonNull<c_void>; Self::FRAME_COUNT] = [core::ptr::NonNull::dangling(); Self::FRAME_COUNT];
            for i in 0..Self::FRAME_COUNT {
                let allocator : ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
                allocators[i] = core::mem::transmute(allocator);   
            }
            Ok(core::mem::transmute(allocators))
        }
    }

    fn create_cmd_list(device: &ID3D12Device, cmd_allocator: &ID3D12CommandAllocator) -> HR<ID3D12GraphicsCommandList> {
        unsafe {
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, cmd_allocator, None)
        }
    }

    fn create_desc_heap(device: &ID3D12Device) -> HR<ID3D12DescriptorHeap> {
        let desc = D3D12_DESCRIPTOR_HEAP_DESC {
            NumDescriptors: Self::FRAME_COUNT as _,
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: 0,
        };
        unsafe {
            device.CreateDescriptorHeap(&desc)
        }
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_DESTROY => {
                PostQuitMessage(0);
                
            },
            _ => {},
        }

        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            if let Ok(_) = self.wait_gpu() {
                if !self.fence_event.is_invalid() {
                    CloseHandle(self.fence_event);
                }
            }
        }
    }
}