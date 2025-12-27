using System;

namespace KairosEngine.Graphics
{
	public class GraphicsShader
	{
		public void* pInternalShader;

		public ~this()
		{
			if(pInternalShader != null)
			{
				GraphicsShader_Dispose(pInternalShader);
				pInternalShader = null;
			}
		}

		public int32 CreateWithoutErrorInfo(String path, ShaderType type)
		{
			char16* pathUtf16;
			TextUtils.Utf8ToUtf16Scope!(path, pathUtf16);
			int32 hr = GraphicsShader_CreateWithoutErrorInfo(&pInternalShader, pathUtf16, type, ShaderCompileFlags.DEBUG | ShaderCompileFlags.SKIP_OPTIMIZATION);
			return hr;
		}
	}
}