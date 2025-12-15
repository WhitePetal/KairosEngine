using System;
using System.Numerics;
using KairosEngine.Editor;

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


			WindowSystem.Instance.Update();

			WindowSystem.Instance.DeInitialize();

			return;
		}
	}
}