using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsResource
	{
		private void* m_pResource;

		public void Dispose() mut
		{
			if(m_pResource != null)
				GraphicsResource_Dispose(&this);
		}
	}
}