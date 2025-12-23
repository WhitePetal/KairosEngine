using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsPipelineState
	{
		private void* m_pPipelineState;

		public void Dispose() mut
		{
			if(m_pPipelineState != null)
				GraphicsPipelineState_Dispose(&this);
		}
	}
}