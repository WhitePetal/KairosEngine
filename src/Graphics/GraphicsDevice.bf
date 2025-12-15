using System;

namespace KairosEngine.Graphics
{
	public struct GraphicsDevice
	{
		private Pointer m_pDevice;

		public this()
		{
			m_pDevice = GraphicsDevice_Create();
		}

		public int Initialize()
		{
			return GraphicsDevice_Initialize(m_pDevice);
		}

		public void DeInitialize()
		{
			Graphics_DeInitialize(m_pDevice);
		}
	}
}