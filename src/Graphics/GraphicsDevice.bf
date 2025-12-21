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
			return (result.HR, GraphicsCommandQueue(result.Ptr));
		}

		public (int hr, GraphicsDescriptorHeap heap) CreateDescriptorHeap(int count, DescriptorHeapType type, DescriptorHeapFlags flags)
		{
			CreateResult result = GraphicsDevice_CreateDescriptorHeap(m_pDevice, count, type, flags);
			return (result.HR, GraphicsDescriptorHeap(result.Ptr, type, flags));
		}

		public uint GetDescriptorHandleIncrementSize(DescriptorHeapType type)
		{
			return GraphicsDevice_GetDescriptorHandleIncrementSize(m_pDevice, type);
		}

		public void CreateRenderTargetView(GraphicsRenderTarget renderTarget, GraphicsCPUDescriptorHandle handle)
		{
			GraphicsDevice_CreateRenderTargetView(m_pDevice, renderTarget.GetInternalPtr(), handle.Ptr);
		}

		public void CreateRenderTargetView(GraphicsRenderTarget renderTarget, GraphicsDescriptorHeap descriptorHeap, int index)
		{
			GraphicsDevice_CreateRenderTargetView(m_pDevice, renderTarget.GetInternalPtr(), descriptorHeap.GetInternalPtr(), index);
		}
	}
}