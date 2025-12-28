using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct ShaderByteCode
	{
		public void* pShaderBytecode;
		public uint64 BytecodeLenth;

		public this(GraphicsShader shader)
		{
			pShaderBytecode = shader.GetBufferPointer();
			BytecodeLenth = shader.GetBufferSize();
		}
	}
}