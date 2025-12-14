using System;
using System.Numerics;

namespace KairosEngine
{
	class Program
	{
		public static void Main()
		{
			Console.WriteLine("KairosEngine Start");

			var hInstance = Windows.GetModuleHandleW(null);

			WindowSystem windowSys = scope WindowSystem();
			windowSys.Initialize();

			WindowSystem.WndProcDelegate wndProc = scope => WndProc;
			int windId = windowSys.CreateWindow(hInstance, int32_4(0, 0, 800, 600), false, "KairosEngine", "Kairos Window", wndProc);
			if(windId < 0)
			{
				windowSys.DeInitialize();
				return;
			}

			windowSys.Update();

			windowSys.DeInitialize();
			return;
		}

		public static int WndProc(Windows.HWnd hwnd, uint msg, uint64 wParam, uint64 lParam)
		{
			return 0;
		}
	}
}