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

			int windId = WindowSystem.Instance.CreateWindow(hInstance, int32_4(0, 0, 800, 600), false, "KairosEngine", "Kairos Window", wndProc);
			if(windId < 0)
			{
				WindowSystem.Instance.DeInitialize();
				return;
			}
			editorMainWnd.Id = windId;

			GraphicsDevice device = GraphicsDevice();
			int hr = device.Create();
			if(hr > 0)
				Console.WriteLine("ERROR Create Graphics Device Failed");

			Console.WriteLine("Device Created");

			GraphicsCommandQueue commandQueue = device.CreateaCommandQueue(CommandListType.Direct, 0, CommandQueueFlags.None, 0u);
			/*Console.WriteLine("CommandQueue Created");*/


			WindowSystem.Instance.Update();

			WindowSystem.Instance.DeInitialize();

			commandQueue.Dispose();
			device.Dispose();
			/*delete device;*/
			/*delete commandQueue;*/
			return;
		}
	}
}