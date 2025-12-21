using System;

namespace KairosEngine.Graphics
{
	public struct GraphicsDevice : IDisposable
	{
		private void* m_pDevice;

		public this(void* pDevice)
		{
			m_pDevice = pDevice;
		}

		public void Dispose()
		{
			if(m_pDevice != null)
				GraphicsDevice_Dispose(m_pDevice);
		}

		public (int hr, GraphicsCommandQueue queue) CreateaCommandQueue(CommandListType type, int priority, CommandQueueFlags flags, uint nodeMask)
		{
			CreateResult result = GraphicsDevice_CreateCommandQueue(m_pDevice, type, priority, flags, nodeMask);
			GraphicsCommandQueue commandQueue = GraphicsCommandQueue(result.Ptr);
			return (result.HR, commandQueue);
		}
	}
}