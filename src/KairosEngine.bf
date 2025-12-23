using System;
using System.Numerics;
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

			KairosEditorMainWindow editorMainWnd = new KairosEditorMainWindow();
			defer delete editorMainWnd;

			int32_4 wndRect = int32_4(0, 0, 800, 600);
			int windId = WindowSystem.Instance.CreateWindow(hInstance, wndRect, false, "KairosEngine", "Kairos Window");
			if(windId < 0)
			{
				WindowSystem.Instance.DeInitialize();
				return;
			}
			editorMainWnd.Id = windId;

			GraphicsFactory graphicsFactory = GraphicsFactory();
			int hr = graphicsFactory.Create();
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

			int backBufferCount = 3;

			(hr, GraphicsSwapChain swapChain) = graphicsFactory.CreateSwapChain(ref commandQueue, wndRect.z, wndRect.w, RenderTargetFormat.R8G8B8A8_UNORM, 1, 0, backBufferCount, windId);
			defer swapChain.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Swap Chain Failed");
				return;
			}

			(hr, GraphicsDescriptorHeap rtvHeap) = device.CreateDescriptorHeap(backBufferCount, DescriptorHeapType.RTV, DescriptorHeapFlags.NONE);
			defer rtvHeap.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create RTV DescriptorHeap Failed");
				return;
			}

			GraphicsCPUDescriptorHandle rtvHandle = rtvHeap.GetCPUDescriptorHandleForHeapStart();
			uint rtvDescriptorSize = device.GetDescriptorHandleIncrementSize(DescriptorHeapType.RTV);

			GraphicsRenderTarget[] renderTargets = scope GraphicsRenderTarget[backBufferCount](?);
			int successCount;
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
			defer
			{
				for(int i = 0; i < successCount; ++i)
					renderTargets[i].Dispose();
			}
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
			defer
			{
				for(int i = 0; i < successCount; ++i)
					commandAllocators[i].Dispose();
			}
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
			for(successCount = 0; successCount < backBufferCount; ++successCount)
			{
				(hr, fences[successCount]) = device.CreateFence(0u, FenceFlags.NONE);
				if(hr > 0)
				{
					++successCount;
					break;
				}
			}
			defer
			{
				for(int i = 0; i < successCount; ++i)
					fences[i].Dispose();
			}
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

			WindowSystem.Instance.Update();

			WindowSystem.Instance.DeInitialize();

			Console.WriteLine($"KairosEngine Exit");

			return;
		}
	}
}