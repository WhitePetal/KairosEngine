using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsShader
	{
		private void* m_pShader;

		public void Dispose() mut
		{
			if(m_pShader != null)
				GraphicsShader_Dispose(&this);
		}

		public int32 CreateWithoutErrorInfo(String path, ShaderType type) mut
		{
			char16* pathUtf16;
			TextUtils.Utf8ToUtf16Scope!(path, pathUtf16);
			int32 hr = GraphicsShader_CreateWithoutErrorInfo(&this, pathUtf16, type, ShaderCompileFlags.DEBUG | ShaderCompileFlags.SKIP_OPTIMIZATION);
			return hr;
		}
	}
}