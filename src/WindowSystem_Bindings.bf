using System;

namespace KairosEngine
{
	extension WindowSystem
	{
		[Import("DX12Lib.lib"), CallingConvention(.Cdecl), LinkName("InitializeWindow")]
		public static extern bool InitializeWindow(Windows.HModule hInstance, int ShowWnd, int width, int height, bool fullScreen);

		[Import("DX12Lib.lib"), CallingConvention(.Cdecl), LinkName("MainLoop")]
		public static extern bool MainLoop();
	}
}