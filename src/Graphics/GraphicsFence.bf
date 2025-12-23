using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsFence
	{
		private void* m_pFence;

		public void Dispose() mut
		{
			if(m_pFence != null)
				GraphicsFence_Dispose(&this);
		}
	}
}