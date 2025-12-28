using System;
using System.Numerics;
using ImGui;
using KairosEngine.Math;
using KairosEngine.Editor;
using KairosEngine.Graphics;
using KairosEngine.Platform;
using KairosEngine.ImGUI;

namespace KairosEngine
{
	class Program
	{
		// TODO: temp flag
		public static bool Running;

		public static void Main()
		{
			Console.WriteLine("KairosEngine Start");

			// ================== Init Window =======================
			var hInstance = Windows.GetModuleHandleW(null);

			WindowSystem.Initialize();
			defer WindowSystem.Instance.DeInitialize();

			KairosEditorMainWindow editorMainWnd = new KairosEditorMainWindow();
			defer delete editorMainWnd;

			int32_4 wndRect = int32_4(0, 0, 800, 600);
			(editorMainWnd.Id, editorMainWnd.hWnd) = WindowSystem.Instance.CreateWindow(hInstance, wndRect, false, "KairosEngine", "Kairos Window");
			if(editorMainWnd.Id < 0)
			{
				WindowSystem.Instance.DeInitialize();
				return;
			}

			// =============== Init Graphics ========================

			GraphicsFactory graphicsFactory = new GraphicsFactory();
			int32 hr = graphicsFactory.Create();
			defer delete graphicsFactory;
			if(hr > 0)
			{
				Console.WriteLine("ERROR Create Graphics Factory Failed");
				return;
			}

			(hr, GraphicsDevice device) = graphicsFactory.CreateDevice();
			defer delete device;
			if(hr > 0)
			{
				Console.WriteLine("ERROR Create Graphics Device Failed");
				return;
			}

			(hr, GraphicsCommandQueue commandQueue) = device.CreateaCommandQueue(CommandListType.DIRECT, 0, CommandQueueFlags.None, 0u);
			defer delete commandQueue;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Command Queue Failed");
				return;
			}

			int32 backBufferCount = 3;

			RenderTargetFormat renderTargetFormat = RenderTargetFormat.R8G8B8A8_UNORM;
			(hr, GraphicsSwapChain swapChain) = graphicsFactory.CreateSwapChain(commandQueue, wndRect.z, wndRect.w, renderTargetFormat, 1, 0, backBufferCount, editorMainWnd.Id);
			defer delete swapChain;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Swap Chain Failed");
				return;
			}
			uint32 frameIndex = swapChain.GetCurrentBackBufferIndex();

			(hr, GraphicsDescriptorHeap rtvHeap) = device.CreateDescriptorHeap(backBufferCount, DescriptorHeapType.RTV, DescriptorHeapFlags.NONE);
			defer delete rtvHeap;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create RTV DescriptorHeap Failed");
				return;
			}

			DescriptorCpuHandle rtvHandle = rtvHeap.GetCPUDescriptorHandleForHeapStart();
			uint32 rtvDescriptorSize = device.GetDescriptorHandleIncrementSize(DescriptorHeapType.RTV);

			GraphicsRenderTarget[] renderTargets = scope GraphicsRenderTarget[backBufferCount](?);
			int32 successCount;
			for(successCount = 0; successCount < backBufferCount; ++successCount)
			{
				(hr, renderTargets[successCount]) = swapChain.GetRenderTarget(successCount);
				if(hr > 0)
				{
					++successCount;
					break;
				}
				device.CreateRenderTargetView(renderTargets[successCount], rtvHandle);
				rtvHandle.Offset(1, rtvDescriptorSize);
			}
			for(int32 i = 0; i < successCount; ++i)
				defer::delete renderTargets[i];

			if(hr > 0)
			{
				Console.WriteLine("ERROR Get SwapChain RenderTarget Failed");
				return;
			}

			GraphicsCommandAllocator[] commandAllocators = scope GraphicsCommandAllocator[backBufferCount](?);
			for(successCount = 0; successCount < backBufferCount; ++successCount)
			{
				(hr, commandAllocators[successCount]) = device.CreateCommandAllocator(CommandListType.DIRECT);
				if(hr > 0)
				{
					++successCount;
					break;
				}
			}
			for(int32 i = 0; i < successCount; ++i)
				defer::delete commandAllocators[i];
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create CommandAllocator Failed");
				return;
			}

			(hr, GraphicsCommandList commandList) = device.CreateCommandList(commandAllocators[0], CommandListType.DIRECT, 0u);
			defer delete commandList;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create CommandList Failed");
				return;
			}

			GraphicsFence[] fences = scope GraphicsFence[backBufferCount](?);
			uint64[] fenceValues = scope uint64[backBufferCount](?);
			for(successCount = 0; successCount < backBufferCount; ++successCount)
			{
				(hr, fences[successCount]) = device.CreateFence(0u, FenceFlags.NONE);
				fenceValues[successCount] = 0u;
				if(hr > 0)
				{
					++successCount;
					break;
				}
			}
			for(int32 i = 0; i < successCount; ++i)
				defer::delete fences[i];
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Fence Failed");
				return;
			}

			FenceEvent fenceEvent = FenceEvent();
			hr = fenceEvent.Create();
			defer fenceEvent.Close();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Fence Event Failed");
				return;
			}

			(hr, GraphicsRootSignature rootSignature) = device.CreateEmptyRootSignature(RootSignatureFlags.ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT);
			defer delete rootSignature;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create RootSignature Failed");
				return;
			}

			GraphicsShader vertexShader = new GraphicsShader();
			hr = vertexShader.CreateWithoutErrorInfo("./Shaders/VertexShader.hlsl", ShaderType.VertexShader);
			defer delete vertexShader;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Vertex Shader Failed");
				return;
			}
			GraphicsShader fragmentShader = new GraphicsShader();
			fragmentShader.CreateWithoutErrorInfo("./Shaders/FragmentShader.hlsl", ShaderType.FragmentShader);
			defer delete fragmentShader;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Fragment Shader Failed");
				return;
			}

			InputLayoutElementDesc[] inputLayouts = scope InputLayoutElementDesc[]
			(
				InputLayoutElementDesc("POSITION", 0, InputLayoutElementFormat.R32G32B32_FLOAT, 0, 0, InputLayoutElementClass.PER_VERTEX_DATA, 0)
			);
			(hr, GraphicsPipelineState pipelineState) = device.CreatePipelineState(1, PipelineStateFlags.NONE, inputLayouts, rootSignature, vertexShader, fragmentShader, PrimitiveTopologyType.TRIANGLE, RenderTargetFormat.R8G8B8A8_UNORM, DepthStencilFormat.NONE, 1, 0, 0xffffffff);
			defer delete pipelineState;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Pipeline State Failed");
				return;
			}

			float3[] vertices = scope float3[]
			(
				float3(0.0f, 0.5f, 0.5f), float3(0.5f, -0.5f, 0.5f), float3(-0.5f, -0.5f, 0.5f)
			);
			int verticesSize = vertices.Count * sizeof(float3);
			(hr, GraphicsResource vertexBufferDefaultHeap) = device.CreateCommittedBufferResource(HeapType.DEFAULT, verticesSize, HeapFlags.NONE, ResourceStates.COPY_DEST);
			defer delete vertexBufferDefaultHeap;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Vertex Buffer Default Heap Failed");
				return;
			}
			(hr, GraphicsResource vertexBufferUploadHeap) = device.CreateCommittedBufferResource(HeapType.UPLOAD, verticesSize, HeapFlags.NONE, ResourceStates.GENERIC_READ);
			defer delete vertexBufferUploadHeap;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Vertex Buffer Upload Heap Failed");
				return;
			}
			uint64 requireSize = commandList.UpdateSubResources(vertexBufferDefaultHeap, vertexBufferUploadHeap, 0, 0, 1, vertices, verticesSize);
			if(requireSize <= 0)
			{
				Console.WriteLine($"ERROR Update VertexBuffer from upload heap to default heap Failed");
				return;
			}

			commandList.ResourceBarrier(vertexBufferDefaultHeap, ResourceStates.COPY_DEST, ResourceStates.VERTEX_AND_CONSTANT_BUFFER);
			commandList.Close();
			GraphicsCommandList[] executeCommandLists = scope GraphicsCommandList[]( commandList );
			commandQueue.ExecuteCommandLists(executeCommandLists, 1);

			++fenceValues[frameIndex];
			hr = commandQueue.Signal(fences[frameIndex], fenceValues[frameIndex]);
			if(hr > 0)
			{
				Console.WriteLine($"ERROR CommandQueue Signal Failed");
				return;
			}

			VertexBufferView vertexBufferView = VertexBufferView(vertexBufferDefaultHeap.GetGPUVirtualAddress(), sizeof(float3), verticesSize);

			ViewPort viewPort = ViewPort(0, 0, wndRect.z, wndRect.w, 0f, 1f);
			Rect scissorRect = Rect(0, 0, wndRect.z, wndRect.w);


			// ================ Init ImGUI ========================
			ImGui.CHECKVERSION();
			ImGui.CreateContext();
			ImGui.IO* gui_io = ImGui.GetIO();
			gui_io.ConfigFlags |= ImGui.ConfigFlags.NavEnableKeyboard;
			/*gui_io.ConfigFlags |= ImGui.ConfigFlags.NavEnableGamepad;*/
			gui_io.ConfigFlags |= ImGui.ConfigFlags.DockingEnable;
			gui_io.ConfigFlags |= ImGui.ConfigFlags.ViewportsEnable;
			ImGui.StyleColorsDark();

			// init ImGui Win32
			{
				int64 perf_frequency = 0, pref_counter = 0;
				if(!Kernel.QueryPerformanceFrequency(ref perf_frequency))
				{
					Console.WriteLine("ERROR Query PerformanceFrequency Failed");
					return;
				}
				if(!Kernel.QueryPerformanceCounter(ref pref_counter))
				{
					Console.WriteLine("ERROR Query PerformanceCounter Failed");
					return;
				}
			 	ImGuiWin32Data* gui_bd = new ImGuiWin32Data();
				gui_io.BackendPlatformUserData = gui_bd;
				gui_io.BackendPlatformName = "KairosEngine_Win32";
				gui_io.BackendFlags |= ImGui.BackendFlags.HasMouseCursors;
				gui_io.BackendFlags |= ImGui.BackendFlags.HasSetMousePos;
				gui_io.BackendFlags |= ImGui.BackendFlags.PlatformHasViewports;
				gui_io.BackendFlags |= ImGui.BackendFlags.HasMouseHoveredViewport;
				gui_io.BackendFlags |= ImGui.BackendFlags.HasParentViewport;

				gui_bd.hWnd = editorMainWnd.hWnd;
				gui_bd.TicksPerSecond = perf_frequency;
				gui_bd.Time = perf_frequency;
				gui_bd.LastMouseCursor = ImGui.MouseCursor.COUNT;
				if(Kernel.GetKeyboardCodePgae(ref gui_bd.KeyboardCodePage, sizeof(uint32)) == 0)
					gui_bd.KeyboardCodePage = 0;

				ImGui.PlatformIO* gui_platform_io = ImGui.GetPlatformIO();
				gui_platform_io.Monitors.Resize(0);
				/*ImGui.PlatformMonitor*/
			}

			// init ImGui DX12
			{
				(hr, GraphicsDescriptorHeap gui_srv_descriptor_heap) = device.CreateDescriptorHeap(64, DescriptorHeapType.CBV_SRV_UAV, DescriptorHeapFlags.SHADER_VISIBLE);
				if(hr > 0)
				{
					Console.WriteLine($"ERROR GUI SRV DescriptorHeap Create Failed");
					return;
				}
				ImGuiDescriptorHeapAllocator gui_srv_desc_heap_alloc = ImGuiDescriptorHeapAllocator();
				gui_srv_desc_heap_alloc.Create(device, gui_srv_descriptor_heap);

				ImGuiDX12InitInfo gui_init_info = ImGuiDX12InitInfo{};
				gui_init_info.Device = device;
				gui_init_info.CommandQueue = commandQueue;
				gui_init_info.NumFramesInFlight = backBufferCount;
				gui_init_info.RTVFormat = renderTargetFormat;
				gui_init_info.SrvDescriptorHeap = gui_srv_descriptor_heap;
				gui_init_info.SrvDescriptorAllocFn = scope [&](pInfo, OutCpuHandle, OutGpuHandle) =>
				{
					gui_srv_desc_heap_alloc.Alloc(ref OutCpuHandle, ref OutGpuHandle);
				};
				gui_init_info.SrvDescriptorFreeFn = scope [&](pInfo, cpuHandle, gpuHandle) =>
				{
					gui_srv_desc_heap_alloc.Free(cpuHandle, gpuHandle);
				};

				ImGuiDX12Data* gui_bd = new ImGuiDX12Data();
				gui_bd.InitInfo = gui_init_info;
				gui_bd.Device = gui_init_info.Device;
				gui_bd.CommandQueue = gui_init_info.CommandQueue;
				gui_bd.RTVFormat = gui_init_info.RTVFormat;
				gui_bd.DSVFormat = gui_init_info.DSVFormat;
				gui_bd.NumFramesInFlight = uint32(gui_init_info.NumFramesInFlight);
				gui_bd.SrvDescHeap = gui_init_info.SrvDescriptorHeap;

				gui_bd.TearingSupport = false;

				gui_io.BackendRendererUserData = gui_bd;
				gui_io.BackendRendererName = "KairosEngine_DX12";
				gui_io.BackendFlags |= ImGui.BackendFlags.RendererHasVtxOffset;
				gui_io.BackendFlags |= ImGui.BackendFlags.RendererHasTextures;
				/*gui_io.BackendFlags |= ImGui.BackendFlags.RendererHasViewports;*/

				/*if((gui_io.ConfigFlags & ImGui.ConfigFlags.ViewportsEnable) != 0)*/
					/*ImGuiDX12.InitMultiViewportSupport();*/

				ImGui.Viewport* main_viewport = ImGui.GetMainViewport();
				main_viewport.RendererUserData = System.Internal.UnsafeCastToPtr(new ImGuiDX12ViewPortData(gui_bd.NumFramesInFlight));

				gui_bd.Factory = graphicsFactory;
				bool allow_teraing = false;
				gui_bd.Factory.CheckFeatureSupport(Feature.PRESENT_ALLOW_TEARING, &allow_teraing, sizeof(bool));
				gui_bd.TearingSupport = allow_teraing;

				// Create the gui root signature
				{
					DescriptorRange descRange = DescriptorRange
					{
						RangeType = DescriptorRangeType.SRV,
						NumDescriptors = 1,
						BaseShaderRegister = 0,
						RegisterSpace = 0,
						OffsetInDescriptorsFromTableStart = 0
					};

					RootParameter[] param = scope RootParameter[2](?);

					param[0].ParameterType = RootParameterType._32BIT_CONSTANTS;
					param[0].Unio.Constants.ShaderRegister = 0;
					param[0].Unio.Constants.RegisterSpace = 0;
					param[0].Unio.Constants.Num32BitValues = 16;
					param[0].ShaderVisibility = ShaderVisibility.VERTEX;

					param[1].ParameterType = RootParameterType.DESCRIPTOR_TABLE;
					param[1].Unio.DescriptorTable.NumDescriptorRanges = 1;
					param[1].Unio.DescriptorTable.pDescriptorRanges = &descRange;
					param[1].ShaderVisibility = ShaderVisibility.FRAGMENT;

					StaticSamplerDesc[] staticSampler = scope StaticSamplerDesc[1](?);
					staticSampler[0].Filter = SamplerFilter.MIN_MAG_MIP_LINEAR;
					staticSampler[0].AddressU = TextureAddressMode.CLAMP;
					staticSampler[0].AddressV = TextureAddressMode.CLAMP;
					staticSampler[0].AddressW = TextureAddressMode.CLAMP;
					staticSampler[0].MipLODBias = 0.0f;
					staticSampler[0].MaxAnisotropy = 0;
					staticSampler[0].ComparisonFunc = ComparisonFunc.ALWAYS;
					staticSampler[0].BorderColor = StaticBorderColor.TRANSPARENT_BLACK;
					staticSampler[0].MinLOD = 0.0f;
					staticSampler[0].MaxLOD = float.MaxValue;
					staticSampler[0].ShaderRegister = 0;
					staticSampler[0].RegisterSpace = 0;
					staticSampler[0].ShaderVisibility = ShaderVisibility.FRAGMENT;

					RootSignatureDesc desc = RootSignatureDesc
					{
						NumParameters = uint32(param.Count),
						pParameters = param.Ptr,
						NumStaticSamplers = 1,
						pStaticSamplers = staticSampler.Ptr,
						Flags =
							RootSignatureFlags.ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT |
							RootSignatureFlags.DENY_HULL_SHADER_ROOT_ACCESS |
							RootSignatureFlags.DENY_DOMAIN_SHADER_ROOT_ACCESS |
							RootSignatureFlags.DENY_GEOMETRY_SHADER_ROOT_ACCESS
					};

					(hr, gui_bd.RootSignature) = gui_bd.Device.CreateRootSignature(ref desc);
					if(hr > 0)
					{
						Console.WriteLine($"ERROR Create ImGUI RootSignature Failed");
						return;
					}
				}

				// Create the gui pso
				{
					GraphicsShader gui_vertex_shader = new GraphicsShader();
					hr = gui_vertex_shader.CreateWithoutErrorInfo("./Shaders/GuiVertexShader.hlsl", ShaderType.VertexShader);
					defer delete gui_vertex_shader;
					if(hr > 0)
					{
						Console.WriteLine($"ERROR Create Gui Vertex Shader Failed");
						return;
					}
					GraphicsShader gui_fragment_shader = new GraphicsShader();
					gui_fragment_shader.CreateWithoutErrorInfo("./Shaders/GuiFragmentShader.hlsl", ShaderType.FragmentShader);
					defer delete gui_fragment_shader;
					if(hr > 0)
					{
						Console.WriteLine($"ERROR Create Gui Fragment Shader Failed");
						return;
					}

					InputLayoutElementDesc[] gui_inputLayouts = scope InputLayoutElementDesc[]
					(
						InputLayoutElementDesc("POSITION", 0, InputLayoutElementFormat.R32G32_FLOAT, 0, offsetof(ImGui.DrawVert, pos), InputLayoutElementClass.PER_VERTEX_DATA, 0),
						InputLayoutElementDesc("TEXCOOR", 0, InputLayoutElementFormat.R32G32_FLOAT, 0, offsetof(ImGui.DrawVert, uv), InputLayoutElementClass.PER_VERTEX_DATA, 0),
						InputLayoutElementDesc("COLOR", 0, InputLayoutElementFormat.R8G8B8A8_UNORMA, 0, offsetof(ImGui.DrawVert, col), InputLayoutElementClass.PER_VERTEX_DATA, 0),
					);
					InputLayoutDesc inputLayout = InputLayoutDesc(gui_inputLayouts, 3);

					BlendDesc blendState = BlendDesc(false, ref RenderTargetBlendDesc
					{
						BlendEnable = false,
						SrcBlend = Blend.SRC1_ALPHA,
						DstBlend = Blend.INV_SRC_ALPHA,
						BlendOp = BlendOp.ADD,
						SrcBlendAlpha = Blend.ONE,
						DstBlendAlpha = Blend.INV_SRC_ALPHA,
						BlendOpAlpha = BlendOp.ADD,
						RenderTargetWriteMask = uint8(ColorWriteEnable.ALL)
					});

					RasterizerDesc rasterState = RasterizerDesc
					{
						FillMode = FillMode.SOLID,
						CullMode = CullMode.NONE,
						FrontCounterClockwise = false,
						DepthBias = 0,
						DepthBiasClamp = 0f,
						SlopeScaledDepthBias = 0f,
						DepthClipEnable = true,
						MultisampleEnable = false,
						AntialiasedLineEnable = false,
						ForcedSampleCount = 0,
						ConservativeRaster = ConservativeRasterizationMode.OFF
					};

					DepthStencilDesc depthStencilState = DepthStencilDesc
					{
						DepthEnable = false,
						DepthWriteMask = DepthWriteMask.ALL,
						DepthFunc = ComparisonFunc.ALWAYS,
						StencilEnable = false,
						FrontFace = DepthStencilOpDesc
						{
							StencilFailOp = DepthStencilOp.KEEP,
							StencilDepthFailOp = DepthStencilOp.KEEP,
							StencilFunc = ComparisonFunc.ALWAYS
						}
					};
					depthStencilState.BackFace = depthStencilState.FrontFace;

					MsaaDesc aaDesc = MsaaDesc
					{
						Count = 1,
						Quality = 0
					};

					PipelineStateDesc psoDesc = PipelineStateDesc(gui_bd.RootSignature, ref blendState, ref rasterState, ref depthStencilState, ref inputLayout, ref aaDesc,
						ref gui_bd.RTVFormat, 1, gui_bd.DSVFormat, PrimitiveTopologyType.TRIANGLE, PipelineStateFlags.NONE, uint32.MaxValue, 1, gui_vertex_shader, gui_fragment_shader);
					(hr, gui_bd.PipelineState) = gui_bd.Device.CreatePipelineState(ref psoDesc);
					if(hr > 0)
					{
						Console.WriteLine("ERROR Create GUI PSO Failed");
						return;
					}
				}

				(hr, gui_bd.TexCmdAllocator) = gui_bd.Device.CreateCommandAllocator(CommandListType.DIRECT);
				if(hr > 0)
				{
					Console.WriteLine("ERROR Create GUI Tex CommandAllocator Failed");
					return;
				}
				(hr, gui_bd.TexCmdList) = gui_bd.Device.CreateCommandList(gui_bd.TexCmdAllocator, CommandListType.DIRECT, 0);
				if(hr > 0)
				{
					Console.WriteLine("ERROR Create GUI Tex CommandList Failed");
					return;
				}
				hr = gui_bd.TexCmdList.Close();
				if(hr > 0)
				{
					Console.WriteLine("ERROR Close GUI Tex CommandList Failed");
					return;
				}

				(hr, gui_bd.Fence) = gui_bd.Device.CreateFence(0, FenceFlags.NONE);
				if(hr > 0)
				{
					Console.WriteLine("ERROR Create GUI Fence Failed");
					return;
				}
				gui_bd.FenceEvent = FenceEvent();
			}

			// ================ Engine Loop =========================

			MSG msg = MSG();
			MSG* pMsg = &msg;
			Kernel.InitMSG(pMsg);
			Running = true;

			void WaitPresent()
			{
				frameIndex = swapChain.GetCurrentBackBufferIndex();

				var fence = fences[frameIndex];
				var fenceValue = fenceValues[frameIndex];
				if(fence.GetCompletedValue() < fenceValue)
				{
					hr = fence.SetEventOnCompletion(fenceEvent, fenceValue);
					if(hr > 0)
					{
						Console.WriteLine($"ERROR Set Fence Completion Event Failed");
					 	Running = false;
						return;
					}

					fenceEvent.Wait(FenceEvent.INFINITE_WAIT_TIME);
				}

				++fenceValues[frameIndex];
			}

			void RenderLoop()
			{
				WaitPresent();

				hr = commandAllocators[frameIndex].Reset();
				if(hr > 0)
				{
					Console.WriteLine($"ERROR CommandAllocator Reset Failed");
					Running = false;
					return;
				}
				hr = commandList.Reset(commandAllocators[frameIndex], pipelineState);
				if(hr > 0)
				{
					Console.WriteLine($"ERROR CommandList Reset Failed");
					Running = false;
					return;
				}

				commandList.ResourceBarrier(renderTargets[frameIndex], ResourceStates.PRESENT, ResourceStates.RENDER_TARGET);
				commandList.OMSetRenderTargets(rtvHeap, frameIndex, rtvDescriptorSize, 1);
				commandList.ClearRenderTargetView(rtvHeap, frameIndex, rtvDescriptorSize, ref float4(0f, 0.2f, 0.4f, 1.0f), 0, null);
				commandList.ResourceBarrier(renderTargets[frameIndex], ResourceStates.RENDER_TARGET, ResourceStates.PRESENT);
				hr = commandList.Close();
				if(hr > 0)
				{
					Console.WriteLine($"ERROR CommandList Close Failed");
					Running = false;
					return;
				}

				GraphicsCommandList[] commandLists = scope GraphicsCommandList[](commandList);
				commandQueue.ExecuteCommandLists(commandLists, 1);
				hr = commandQueue.Signal(fences[frameIndex], fenceValues[frameIndex]);
				if(hr > 0)
				{
					Console.WriteLine($"ERROR CommandQueue Signal Failed");
					Running = false;
					return;
				}

				hr = swapChain.Present(0u, 0u);
				if(hr > 0)
				{
					Console.WriteLine($"ERROR SwapChain Present Failed");
					Running = false;
					return;
				}
			}

			void RenderUI()
			{
				hr = ImGuiDX12.NewFrame();
				if(hr > 0)
				{
					Console.WriteLine("ERROR GUI DX12 NewFrame Failed");
					Running = false;
					return;
				}
				ImGui.NewFrame();
			}

			while(Running)
			{
				if(Kernel.PeekMessage(pMsg) == 1)
				{
					if(msg.message == 0x0012)
						break;

					Kernel.TranslateMessage(pMsg);
					Kernel.DispatchMessage(pMsg);
				}
				else
				{
					WindowSystem.Instance.Update();
					// do game logic
					RenderLoop();
					RenderUI();
				}
			}

			Console.WriteLine($"KairosEngine Exit");

			return;
		}
	}
}