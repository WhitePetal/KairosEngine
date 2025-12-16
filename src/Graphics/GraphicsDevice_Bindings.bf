using System;

namespace KairosEngine.Graphics
{
	extension GraphicsDevice
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_Allocate")]
		private static extern void* GraphicsDevice_Allocate();

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_Create")]
		private static extern int GraphicsDevice_Create(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_Dispose")]
		private static extern void GraphicsDevice_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandQueue")]
		public static extern CreateResult GraphicsDevice_CreateCommandQueue(void* _this, CommandListType type, int priority, CommandQueueFlags flags, uint nodeMask);
	}
}