using System;

namespace KairosEngine
{
	extension WindowSystem
	{
		[Import("KairosEngine.dll"), LinkName("InitializeWindow")]
		public static extern bool InitializeWindow(Windows.HModule hInstance, int ShowWnd, int width, int height, bool fullScreen);

		[Import("KairosEngine.dll"), LinkName("MainLoop")]
		public static extern bool MainLoop();
	}
}