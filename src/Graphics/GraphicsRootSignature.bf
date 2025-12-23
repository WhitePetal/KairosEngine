using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsRootSignature
	{
		private void* m_pRootSignature;

		public void Dispose() mut
		{
			if(m_pRootSignature != null)
				GraphicsRootSignature_Dispose(&this);
		}
	}
}