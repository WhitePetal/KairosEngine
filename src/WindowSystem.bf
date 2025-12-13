using System;
using System.Numerics;

namespace KairosEngine
{
	public class WindowSystem
	{
		private WindowComponents m_Components;

		public void Initialize()
		{
			WindowComponents.Initialize();
			m_Components = WindowComponents.Instance;
		}

		public void DeInitialize()
		{
			m_Components.DeInitialize();
		}

		public int CreateWindow(Windows.HModule hInstanc, int32_4 rect, bool fullScreen = false)
		{
			int id = m_Components.CreateWindow(rect, fullScreen);

			if(!InitializeWindow(hInstanc, Windows.SW_SHOWDEFAULT, rect.z, rect.w, fullScreen))
			{
				Console.WriteLine($"Window Initialization - Failed, Window Id: {id}");
				return -1;
			}

			return id;
		}

		public void Update()
		{
			MainLoop();
			/*for(int i = 0; i < m_Components.WindowCount; ++i)
			{

			}*/
		}
	}
}