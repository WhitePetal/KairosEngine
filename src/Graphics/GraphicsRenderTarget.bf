using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsRenderTarget
	{
		private void* m_pRenderTarget;

		public void Dispose() mut
		{
			if(m_pRenderTarget != null)
				GraphicsRenderTarget_Dispose(&this);
		}
	}
}