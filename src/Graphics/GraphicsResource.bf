using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsResource
	{
		protected void* m_pResource;

		public void Dispose() mut
		{
			if(m_pResource != null)
				GraphicsResource_Dispose(&this);
		}

		public uint64 GetGPUVirtualAddress() mut
		{
			return GraphicsResource_GetGPUVirtualAddress(&this);
		}
	}
}