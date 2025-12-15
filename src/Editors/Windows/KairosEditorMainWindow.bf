using System;

namespace KairosEngine.Editor
{
	class KairosEditorMainWindow
	{
		public int Id;

		public int64 WndProc(Windows.HWnd hWnd, uint msg, uint64 wParam, int64 lParam)
		{
			switch (msg)
			{
			case 0x0100:
				if (wParam == 0x1B)
						WindowSystem.Instance.DestroyWindow(Id);
				return 0;
			case 0x0002:
				WindowSystem.KairosPostQuitMessage(0);
				return 0;
			}
			return WindowSystem.KairosDefWindowProcW(hWnd, msg, wParam, lParam);
		}
	}
}