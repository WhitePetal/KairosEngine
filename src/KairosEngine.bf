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

			(hr, GraphicsSwapChain swapChain) = graphicsFactory.CreateSwapChain(commandQueue, wndRect.z, wndRect.w, RenderTargetFormats.R8G8B8A8_UNORM, 1, 0, 3, windId);
			defer swapChain.Dispose();
			if(hr > 0)
			{
				Console.WriteLine($"ERROR Create Swap Chain Failed");
				return;
			}

			Console.WriteLine($"Current Back Buffer Index: {swapChain.GetCurrentBackBufferIndex()}");


			WindowSystem.Instance.Update();

			WindowSystem.Instance.DeInitialize();

			return;
		}
	}
}