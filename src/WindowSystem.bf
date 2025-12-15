using System;
using System.Numerics;

namespace KairosEngine
{
	public class WindowSystem
	{
		public static WindowSystem Instance;
		private WindowComponents m_Components;

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

		public int CreateWindow(Windows.HModule hInstanc, int32_4 rect, bool fullScreen, String windowName, String windowTitle, WndProcPtr wndProc)
		{
			char16* wndName;
			char16* wndTitle;
			TextUtils.Utf8ToUtf16Scope!(windowName, wndName);
			TextUtils.Utf8ToUtf16Scope!(windowTitle, wndTitle);

			Windows.HWnd hwnd = KairosInitializeWindow(hInstanc, Windows.SW_SHOWDEFAULT, rect.z, rect.w, fullScreen, wndName, wndTitle, wndProc);
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

		}

		public void Update()
		{
			KairosMainLoop();
		}
	}
}