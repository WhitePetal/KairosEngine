using System;

namespace KairosEngine.Graphics
{
	public class GraphicsSwapChain
	{
		public void* pInternalSwapChain;

		public ~this()
		{
			if(pInternalSwapChain != null)
			{
				GraphicsSwapChain_Dispose(pInternalSwapChain);
				pInternalSwapChain = null;
			}
		}

		public uint32 GetCurrentBackBufferIndex()
		{
			return GraphicsSwapChain_GetCurrentBackBufferIndex(pInternalSwapChain);
		}

		public (int32 hr, GraphicsRenderTarget renderTarget) GetRenderTarget(int32 index)
		{
			GraphicsRenderTarget renderTarget = new GraphicsRenderTarget();
			int32 hr = GraphicsSwapChain_GetRenderTarget(pInternalSwapChain, &renderTarget.pInternalResource, index);
			return (hr, renderTarget);
		}

		public int32 Present(uint32 syncInternal, uint32 flags)
		{
			return GraphicsSwapChain_Present(pInternalSwapChain, syncInternal, flags);
		}
	}
}