using System;

namespace KairosEngine.Graphics
{
	extension GraphicsRootSignature
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsRootSignature_Dispose")]
		private static extern void GraphicsRootSignature_Dispose(GraphicsRootSignature* _this);
	}
}