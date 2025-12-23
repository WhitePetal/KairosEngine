using System;
using System.Text;


namespace KairosEngine
{
	public static class TextUtils
	{
		public static mixin Utf8ToUtf16Scope(String str, char16* lpstr)
		{
			int32 maxLen = int32(str.Length) * 2 + 1;
			char16[] lchars = scope:mixin char16[maxLen];
			int32 len = int32(System.Text.UTF16.Encode(str, &lchars[0], maxLen).Value);
			lchars[len] = '\0';
			lpstr = &lchars[0];
		}
	}
}