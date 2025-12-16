using System;

namespace KairosEngine.Graphics
{
	enum CommandQueueFlags : uint
	{
		None = 0,
		DisableGpuTimeout = 0x1
	}

	struct GraphicsCommandQueue : IDisposable
	{
		private void* m_pCommandQueue;

		public this(void* pCommandQueue)
		{
			m_pCommandQueue = pCommandQueue;
		}

		public void Dispose()
		{
			GraphicsCommandQueue_Dispose(m_pCommandQueue);
		}
	}
}