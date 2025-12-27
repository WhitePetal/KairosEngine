using System;

namespace KairosEngine.Graphics
{
	public class GraphicsRootSignature
	{
		public void* pInternalRootSignature;

		public ~this()
		{
			if(pInternalRootSignature != null)
			{
				GraphicsRootSignature_Dispose(pInternalRootSignature);
				pInternalRootSignature = null;
			}
		}
	}
}