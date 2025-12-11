using System;

namespace KairosEngine
{
	class KairosEngine
	{

	}

	class Program
	{
		[Import("KairosEngine.dll"), LinkName("InitializeWindow")]
		static extern bool InitializeWindow(Windows.HModule hInstance, int ShowWnd, int width, int height, bool fullScreen);

		[Import("KairosEngine.dll"), LinkName("Kairos_MainLoop")]
		static extern bool MainLoop();

		public static void Main()
		{
			Console.WriteLine("KairosEngine Start");
			var hInstance = Windows.GetModuleHandleW(null);
			if(!InitializeWindow(hInstance, Windows.SW_SHOWDEFAULT, 800, 600, false))
			{
				Console.WriteLine("Window Initialization - Failed");
				return;
			}

			MainLoop();
			return;
		}
	}
}