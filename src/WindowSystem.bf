using System;
using System.Numerics;

namespace KairosEngine
{
	public class WindowSystem
	{
		public static WindowSystem Instance;
		private WindowComponents m_Components;

		// TOOD: temp flag
		public bool Running;

		public static void Initialize()
		{
			WindowComponents.Initialize();
			Instance = new WindowSystem();
			Instance.m_Components = WindowComponents.Instance;
		}

		public void DeInitialize()
		{
			m_Components.DeInitialize();
			delete Instance;
		}

		public static int64 WndProc(Windows.HWnd hWnd, uint msg, uint64 wParam, int64 lParam)
		{
			int wndCount = WindowComponents.Instance.WindowCount;
			for(int i = 0; i < wndCount; ++i)
			{
				if(WindowComponents.Instance.Hwnds[i] != hWnd)
					continue;

				switch (msg)
				{
				case 0x0100:
					if (wParam == 0x1B)
					{
						int id = WindowComponents.Instance.Ids[i];
						WindowSystem.Instance.DestroyWindow(id);
						WindowSystem.Instance.Running = false;
					}
					return 0;
				case 0x0002:
					WindowSystem.KairosPostQuitMessage(0);
					return 0;
				}
			}

			return WindowSystem.KairosDefWindowProcW(hWnd, msg, wParam, lParam);
		}

		public int CreateWindow(Windows.HModule hInstanc, int32_4 rect, bool fullScreen, String windowName, String windowTitle)
		{
			char16* wndName;
			char16* wndTitle;
			TextUtils.Utf8ToUtf16Scope!(windowName, wndName);
			TextUtils.Utf8ToUtf16Scope!(windowTitle, wndTitle);

			Windows.HWnd hwnd = KairosInitializeWindow(hInstanc, Windows.SW_SHOWDEFAULT, rect.z, rect.w, fullScreen, wndName, wndTitle, WndProcCallback);
			Console.WriteLine($"Window hWnd: {(int)hwnd}");
			if(hwnd == 0)
			{
				Console.WriteLine($"Window Initialization - Failed");
				return -1;
			}
			int id = m_Components.CreateWindow(rect, fullScreen, hwnd);

			return id;
		}

		public void DestroyWindow(int id)
		{
			int index = m_Components.IdToIndex[id];
			Windows.HWnd hwnd = m_Components.Hwnds[index];
			m_Components.DestroyWindow(id);
			KairosDestroyWindow(hwnd);
		}

		public void Update()
		{
			MSG msg = MSG();
			MSG* pMsg = &msg;
			KairosInitMSG(pMsg);
			Running = true;
			while(Running)
			{
				if(KairosPeekMessage(pMsg) == 1)
				{
					if(msg.message == 0x0012)
						break;

					KairosTranslateMessage(pMsg);
					KairosDispatchMessage(pMsg);
				}
				else
				{
					// do game logic
				}
			}
		}
	}
}