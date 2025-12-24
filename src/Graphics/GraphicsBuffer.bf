using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsBuffer
	{
		private void* m_pResource;

		public void Dispose() mut
		{
			if(m_pResource != null)
				GraphicsBuffer_Dispose(&this);
		}
	}
}