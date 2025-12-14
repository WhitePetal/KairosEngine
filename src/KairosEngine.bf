using System;
using System.Numerics;

namespace KairosEngine
{
	class Program
	{
		private static Windows.WndProc s_WndProc = => WndProc;
		public static void Main()
		{
			Console.WriteLine("KairosEngine Start");

			var hInstance = Windows.GetModuleHandleW(null);

			WindowSystem.Initialize();

			int windId = WindowSystem.Instance.CreateWindow(hInstance, int32_4(0, 0, 800, 600), false, "KairosEngine", "Kairos Window", s_WndProc);
			if(windId < 0)
			{
				WindowSystem.Instance.DeInitialize();
				return;
			}


			WindowSystem.Instance.Update();

			WindowSystem.Instance.DeInitialize();

			return;
		}

		public static int WndProc(Windows.HWnd hWnd, int32 msg, int wParam, int lParam)
		{
			switch (msg)
			{
			case 0x0100:
				if (wParam == 0x1B)
						WindowSystem.KairosDestroyWindow(hWnd);
				return 0;
			case 0x0002:
				WindowSystem.KairosPostQuitMessage(0);
				return 0;
			}
			return WindowSystem.KairosDefWindowProcW(hWnd, msg, wParam, lParam);
		}
	}
}