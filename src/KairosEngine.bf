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
			WindowSystem.WndProcPtr wndProc = new => editorMainWnd.WndProc;

			defer delete wndProc;
			defer delete editorMainWnd;

			int32_4 wndRect = int32_4(0, 0, 800, 600);
			int windId = WindowSystem.Instance.CreateWindow(hInstance, wndRect, false, "KairosEngine", "Kairos Window", wndProc);
			if(windId < 0)
			{
				WindowSystem.Instance.DeInitialize();
				return;
			}
			editorMainWnd.Id = windId;

			GraphicsFactory graphicsFactory = GraphicsFactory();
			defer graphicsFactory.Dispose();
			int hr = graphicsFactory.Create();
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

			(hr, GraphicsCommandQueue commandQueue) = device.CreateaCommandQueue(CommandListType.Direct, 0, CommandQueueFlags.None, 0u);
			defer commandQueue.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Command Queue Failed");
				return;
			}

			int backBufferCount = 3;

			(hr, GraphicsSwapChain swapChain) = graphicsFactory.CreateSwapChain(commandQueue, wndRect.z, wndRect.w, RenderTargetFormat.R8G8B8A8_UNORM, 1, 0, backBufferCount, windId);
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
			int getRenderTargetCount = 0;
			for(; getRenderTargetCount < backBufferCount; ++getRenderTargetCount)
			{
				(hr, renderTargets[getRenderTargetCount]) = swapChain.GetRenderTarget(getRenderTargetCount);
				if(hr > 0)
				{
					++getRenderTargetCount;
					break;
				}
				device.CreateRenderTargetView(renderTargets[getRenderTargetCount], rtvHandle);
				rtvHandle.Offset(1, rtvDescriptorSize);
			}
			defer
			{
				for(int i = 0; i < getRenderTargetCount; ++i)
				{
					renderTargets[i].Dispose();
				}
			}
			if(hr > 0)
			{
				Console.WriteLine("ERROR Get SwapChain RenderTarget Failed");
				return;
			}

			GraphicsCommandAllocator[] commandAllocators = scope GraphicsCommandAllocator[backBufferCount](?);
			int createCommandAllocatorCount = 0;
			for(; createCommandAllocatorCount < backBufferCount; ++createCommandAllocatorCount)
			{
				(hr, commandAllocators[createCommandAllocatorCount]) = device.CreateCommandAllocator(CommandListType.Direct);
				if(hr > 0)
				{
					++createCommandAllocatorCount;
					break;
				}
			}
			defer
			{
				for(int i = 0; i < createCommandAllocatorCount; ++i)
				{
					commandAllocators[i].Dispose();
				}
			}
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create CommandAllocator Failed");
				return;
			}

			(hr, GraphicsCommandList commandList) = device.CreateCommandList(commandAllocators[0], CommandListType.Direct, 0u);
			defer commandList.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create CommandList Failed");
				return;
			}


			WindowSystem.Instance.Update();

			WindowSystem.Instance.DeInitialize();

			return;
		}
	}
}