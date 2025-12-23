using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsCommandAllocator
	{
		private void* m_pCommandAllocator;

		public void Dispose() mut
		{
			if(m_pCommandAllocator != null)
				GraphicsCommandAllocator_Dispose(&this);
		}
	}
}