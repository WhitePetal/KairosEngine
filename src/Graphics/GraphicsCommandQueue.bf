using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsCommandQueue : IDisposable
	{
		private void* m_pCommandQueue;

		public void Dispose() mut
		{
			if(m_pCommandQueue != null)
			 	GraphicsCommandQueue_Dispose(&this);
		}
	}
}