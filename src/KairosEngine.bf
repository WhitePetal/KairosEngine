using System;
using System.Numerics;
using KairosEngine.Math;
using KairosEngine.Editor;
using KairosEngine.Graphics;

namespace KairosEngine
{
	class Program
	{
		public static void Main()
		{
			Console.WriteLine("KairosEngine Start");

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

			GraphicsFactory graphicsFactory = GraphicsFactory();
			int32 hr = graphicsFactory.Create();
			defer graphicsFactory.Dispose();
			if(hr > 0)
			{
				Console.WriteLine("ERROR Create Graphics Factory Failed");
				return;
			}

			(hr, GraphicsDevice device) = graphicsFactory.CreateDevice();
			defer device.Dispose();
			if(hr > 0)
			{
				Console.WriteLine("ERROR Create Graphics Device Failed");
				return;
			}

			(hr, GraphicsCommandQueue commandQueue) = device.CreateaCommandQueue(CommandListType.DIRECT, 0, CommandQueueFlags.None, 0u);
			defer commandQueue.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Command Queue Failed");
				return;
			}

			int32 backBufferCount = 3;

			(hr, GraphicsSwapChain swapChain) = graphicsFactory.CreateSwapChain(ref commandQueue, wndRect.z, wndRect.w, RenderTargetFormat.R8G8B8A8_UNORM, 1, 0, backBufferCount, windId);
			defer swapChain.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Swap Chain Failed");
				return;
			}
			uint32 frameIndex = swapChain.GetCurrentBackBufferIndex();

			(hr, GraphicsDescriptorHeap rtvHeap) = device.CreateDescriptorHeap(backBufferCount, DescriptorHeapType.RTV, DescriptorHeapFlags.NONE);
			defer rtvHeap.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create RTV DescriptorHeap Failed");
				return;
			}

			GraphicsCPUDescriptorHandle rtvHandle = rtvHeap.GetCPUDescriptorHandleForHeapStart();
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
				device.CreateRenderTargetView(ref renderTargets[successCount], rtvHandle);
				rtvHandle.Offset(1, rtvDescriptorSize);
			}
			for(int32 i = 0; i < successCount; ++i)
				defer::renderTargets[i].Dispose();

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
				defer::commandAllocators[i].Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create CommandAllocator Failed");
				return;
			}

			(hr, GraphicsCommandList commandList) = device.CreateCommandList(ref commandAllocators[0], CommandListType.DIRECT, 0u);
			defer commandList.Dispose();
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
				defer::fences[i].Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Fence Failed");
				return;
			}

			GraphicsFenceEvent fenceEvent = GraphicsFenceEvent();
			hr = fenceEvent.Create();
			defer fenceEvent.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Fence Event Failed");
				return;
			}

			(hr, GraphicsRootSignature rootSignature) = device.CreateEmptyRootSignature(RootSignatureFlags.ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT);
			defer rootSignature.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create RootSignature Failed");
				return;
			}

			GraphicsShader vertexShader = GraphicsShader();
			hr = vertexShader.CreateWithoutErrorInfo("./Shaders/VertexShader.hlsl", ShaderType.VertexShader);
			defer vertexShader.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Vertex Shader Failed");
				return;
			}
			GraphicsShader fragmentShader = GraphicsShader();
			fragmentShader.CreateWithoutErrorInfo("./Shaders/PixelShader.hlsl", ShaderType.FragmentShader);
			defer fragmentShader.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Fragment Shader Failed");
				return;
			}

			GraphicsInputLayoutElement[] inputLayouts = scope GraphicsInputLayoutElement[]
			(
				GraphicsInputLayoutElement("POSITION", 0, InputLayoutElementFormat.R32G32B32_FLOAT, 0, 0, InputLayoutElementClass.PER_VERTEX_DATA, 0)
			);
			(hr, GraphicsPipelineState pipelineState) = device.CreatePipelineState(inputLayouts, ref rootSignature, ref vertexShader, ref fragmentShader, TopologyType.TRIANGLE, RenderTargetFormat.R8G8B8A8_UNORM, 1, 0, 0xffffffff);
			defer pipelineState.Dispose();
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
			defer vertexBufferDefaultHeap.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Vertex Buffer Default Heap Failed");
				return;
			}
			(hr, GraphicsResource vertexBufferUploadHeap) = device.CreateCommittedBufferResource(HeapType.UPLOAD, verticesSize, HeapFlags.NONE, ResourceStates.GENERIC_READ);
			defer vertexBufferUploadHeap.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Vertex Buffer Upload Heap Failed");
				return;
			}
			hr = commandList.UpdateSubResources(ref vertexBufferDefaultHeap, ref vertexBufferUploadHeap, 0, 0, 1, vertices, verticesSize);
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Update VertexBuffer from upload heap to default heap Failed");
				return;
			}

			commandList.ResourceBarrier(ref vertexBufferDefaultHeap, ResourceStates.COPY_DEST, ResourceStates.VERTEX_AND_CONSTANT_BUFFER);
			commandList.Close();
			GraphicsCommandList[] executeCommandLists = scope GraphicsCommandList[]( commandList );
			commandQueue.ExecuteCommandLists(executeCommandLists, 1);

			++fenceValues[frameIndex];
			hr = commandQueue.Signal(ref fences[frameIndex], fenceValues[frameIndex]);
			if(hr > 0)
			{
				Console.WriteLine($"ERROR CommandQueue Signal Failed");
				return;
			}

			GraphicsVertexBufferView vertexBufferView = GraphicsVertexBufferView(vertexBufferDefaultHeap.GetGPUVirtualAddress(), sizeof(float3), verticesSize);

			ViewPort viewPort = ViewPort(0, 0, wndRect.z, wndRect.w, 0f, 1f);
			Rect scissorRect = Rect(0, 0, wndRect.z, wndRect.w);

			void RenderLoop()
			{
				
			}

			function void() renderLoopPtr = =>RenderLoop;

			WindowSystem.Instance.Update(renderLoopPtr);

			Console.WriteLine($"KairosEngine Exit");

			return;
		}
	}
}