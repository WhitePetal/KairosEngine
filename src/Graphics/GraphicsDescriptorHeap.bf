namespace KairosEngine.Graphics
{
	public struct GraphicsDescriptorHeap
	{
		private void* m_pDescriptorHeap;

		public DescriptorHeapType Type;
		public DescriptorHeapFlags Flags;

		public this(void* pDescriptorHeap, DescriptorHeapType type, DescriptorHeapFlags flags)
		{
			m_pDescriptorHeap = pDescriptorHeap;
			Type = type;
			Flags = flags;
		}

		public void Dispose()
		{
			if(m_pDescriptorHeap != null)
				GraphicsDescriptorHeap_Dispose(m_pDescriptorHeap);
		}

		public void* GetInternalPtr()
		{
			return m_pDescriptorHeap;
		}

		public GraphicsCPUDescriptorHandle GetCPUDescriptorHandleForHeapStart()
		{
			return GraphicsCPUDescriptorHandle(GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(m_pDescriptorHeap));
		}
	}
}