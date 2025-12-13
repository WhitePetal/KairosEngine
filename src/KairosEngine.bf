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

			int windId = windowSys.CreateWindow(hInstance, int32_4(0, 0, 800, 600), false);
			if(windId < 0)
			{
				windowSys.DeInitialize();
				return;
			}

			windowSys.Update();

			windowSys.DeInitialize();
			return;
		}
	}
}