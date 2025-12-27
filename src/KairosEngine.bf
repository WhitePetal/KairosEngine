using System;
using System.Numerics;
using ImGui;
using KairosEngine.Math;
using KairosEngine.Editor;
using KairosEngine.Graphics;
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
			int32 windId = WindowSystem.Instance.CreateWindow(hInstance, wndRect, false, "KairosEngine", "Kairos Window");
			if(windId < 0)
			{
				WindowSystem.Instance.DeInitialize();
				return;
			}
			editorMainWnd.Id = windId;

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
			(hr, GraphicsSwapChain swapChain) = graphicsFactory.CreateSwapChain(commandQueue, wndRect.z, wndRect.w, renderTargetFormat, 1, 0, backBufferCount, windId);
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
			fragmentShader.CreateWithoutErrorInfo("./Shaders/PixelShader.hlsl", ShaderType.FragmentShader);
			defer delete fragmentShader;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Fragment Shader Failed");
				return;
			}

			InputLayoutElement[] inputLayouts = scope InputLayoutElement[]
			(
				InputLayoutElement("POSITION", 0, InputLayoutElementFormat.R32G32B32_FLOAT, 0, 0, InputLayoutElementClass.PER_VERTEX_DATA, 0)
			);
			(hr, GraphicsPipelineState pipelineState) = device.CreatePipelineState(inputLayouts, rootSignature, vertexShader, fragmentShader, TopologyType.TRIANGLE, RenderTargetFormat.R8G8B8A8_UNORM, 1, 0, 0xffffffff);
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
			/*ImGui.CHECKVERSION();
			ImGui.CreateContext();
			ImGui.IO* gui_io = ImGui.GetIO();
			gui_io.ConfigFlags |= ImGui.ConfigFlags.NavEnableKeyboard;
			gui_io.ConfigFlags |= ImGui.ConfigFlags.NavEnableGamepad;
			defer ImGui.DestroyContext();

			(hr, GraphicsDescriptorHeap gui_srv_descriptor_heap) = device.CreateDescriptorHeap(64, DescriptorHeapType.CBV_SRV_UAV, DescriptorHeapFlags.SHADER_VISIBLE);
			defer delete gui_srv_descriptor_heap;
			if(hr > 0)
			{
				Console.WriteLine($"ERROR GUI SRV DescriptorHeap Create Failed");
				return;
			}
			ImGuiDescriptorHeapAllocator gui_srv_desc_heap_alloc = ImGuiDescriptorHeapAllocator();
			gui_srv_desc_heap_alloc.Create(&device, &gui_srv_descriptor_heap);

			ImGuiDX12InitInfo gui_init_info = ImGuiDX12InitInfo{};
			gui_init_info.pDevice = &device;
			gui_init_info.pCommandQueue = &commandQueue;
			gui_init_info.NumFramesInFlight = backBufferCount;
			gui_init_info.RTVFormat = renderTargetFormat;
			gui_init_info.SrvDescriptorHeap = &gui_srv_descriptor_heap;
			gui_init_info.SrvDescriptorAllocFn = scope [&](pInfo, pOutCpuHandle, pOutGpuHandle) =>
			{
				gui_srv_desc_heap_alloc.Alloc(pOutCpuHandle, pOutGpuHandle);
			};
			gui_init_info.SrvDescriptorFreeFn = scope [&](pInfo, cpuHandle, gpuHandle) =>
			{
				gui_srv_desc_heap_alloc.Free(cpuHandle, gpuHandle);
			};

			ImGuiDX12RenderBuffers[] gui_renderBuffers = scope ImGuiDX12RenderBuffers[backBufferCount](?);
			for(int32 i = 0; i < backBufferCount; ++i)
			{
				ref ImGuiDX12RenderBuffers fr = ref gui_renderBuffers[i];
				fr.pIndexBuffer = null;
				fr.pVertexBuffer = null;
				fr.IndexBufferSize = 10000;
				fr.VertexBufferSize = 5000;
			}
			ImGuiDX12Data gui_bd = ImGuiDX12Data();
			gui_bd.InitInfo = gui_init_info;
			gui_bd.pDevice = gui_init_info.pDevice;
			gui_bd.pCommandQueue = gui_init_info.pCommandQueue;
			gui_bd.RTVFormat = gui_init_info.RTVFormat;
			gui_bd.DSVFormat = gui_init_info.DSVFormat;
			gui_bd.NumFramesInFlight = uint32(gui_init_info.NumFramesInFlight);
			gui_bd.pSrvDescHeap = gui_init_info.SrvDescriptorHeap;
			gui_bd.TearingSupport = false;
			gui_bd.FrameIndex = uint32.MaxValue;
			gui_bd.pFrameResources = gui_renderBuffers.Ptr;

			gui_io.BackendRendererUserData = &gui_bd;
			gui_io.BackendRendererName = "KairosEngine_DX12";
			gui_io.BackendFlags |= ImGui.BackendFlags.RendererHasVtxOffset;
			gui_io.BackendFlags |= ImGui.BackendFlags.RendererHasTextures;
			gui_io.BackendFlags |= ImGui.BackendFlags.RendererHasViewports;

			if((gui_io.ConfigFlags & ImGui.ConfigFlags.ViewportsEnable) != 0)
				ImGuiDX12.InitMultiViewportSupport();*/


			// ================ Engine Loop =========================

			MSG msg = MSG();
			MSG* pMsg = &msg;
			Kernel.KairosInitMSG(pMsg);
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
				if(Kernel.KairosPeekMessage(pMsg) == 1)
				{
					if(msg.message == 0x0012)
						break;

					Kernel.KairosTranslateMessage(pMsg);
					Kernel.KairosDispatchMessage(pMsg);
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