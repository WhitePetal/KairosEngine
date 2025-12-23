using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsCommandList
	{
		private void* m_pGraphicsCommandList;

		public void Dispose() mut
		{
			if(m_pGraphicsCommandList != null)
				GraphicsCommandList_Dispose(&this);
		}
	}
}