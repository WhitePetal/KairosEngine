using System;

namespace KairosEngine.Graphics
{
	public struct GraphicsDevice : IDisposable
	{
		private void* m_pDevice;

		public this()
		{
			m_pDevice = GraphicsDevice_Allocate();
		}

		public int Create()
		{
			return GraphicsDevice_Create(m_pDevice);
		}

		public void Dispose()
		{
			GraphicsDevice_Dispose(m_pDevice);
		}

		public GraphicsCommandQueue CreateaCommandQueue(CommandListType type, int priority, CommandQueueFlags flags, uint nodeMask)
		{
			CreateResult result = GraphicsDevice_CreateCommandQueue(m_pDevice, type, priority, flags, nodeMask);
			if(result.HR > 0)
				Console.WriteLine("ERROR:: Create CommandQueue Failed");
			Console.WriteLine("Create CommandQueue Success");
			GraphicsCommandQueue commandQueue = GraphicsCommandQueue(result.Ptr);
			return commandQueue;
		}
	}
}