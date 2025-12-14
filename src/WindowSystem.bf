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

		public int CreateWindow(Windows.HModule hInstanc, int32_4 rect, bool fullScreen, String windowName, String windowTitle, Windows.WndProc wndProc)
		{
			Windows.HWnd hwnd = KairosInitializeWindow(hInstanc, Windows.SW_SHOWDEFAULT, rect.z, rect.w, fullScreen, windowName, windowTitle, wndProc);
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