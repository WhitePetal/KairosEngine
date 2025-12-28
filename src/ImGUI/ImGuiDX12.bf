using ImGui;
using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public static class ImGuiDX12
	{
		public static ImGuiDX12Data* GetBackendData()
		{
			return ImGui.GetCurrentContext() != null ? (ImGuiDX12Data*)ImGui.GetIO().BackendRendererUserData : null;
		}

		public static void InvalidateDeviceObjects()
		{
			ImGuiDX12Data* bd = GetBackendData();
			if(bd == null || bd.Device == null)
				return;

			Safty.DELETE!(bd.Factory);
			Safty.DELETE!(bd.CommandQueue);
			bd.CommandQueueOwned = false;
			Safty.DELETE!(bd.RootSignature);
			Safty.DELETE!(bd.PipelineState);
			if(bd.pTexUploadBufferMapped != null)
			{
				bd.TexUploadBuffer.Unmap(0, 0, bd.pTexUploadBufferSize);
				bd.pTexUploadBufferMapped = null;
			}
			Safty.DELETE!(bd.TexUploadBuffer);
			Safty.DELETE!(bd.TexCmdList);
			Safty.DELETE!(bd.TexCmdAllocator);
			Safty.DELETE!(bd.Fence);
			bd.FenceEvent.Close();
			bd.FenceEvent = default;

			// destroy all textures
			var textures = ImGui.GetPlatformIO().Textures;
			for(int32 i = 0; i < textures.Size; ++i)
			{
				ImGui.TextureData* tex = textures.Data[i];
				if(tex.RefCount == 1)
					DestroyTexture(tex);
			}
		}

		public static void DestroyTexture(ImGui.TextureData* tex)
		{
			ImGuiDX12Texture* backend_tex = (ImGuiDX12Texture*)tex.BackendUserData;
			if(backend_tex != null)
			{
				ImGuiDX12Data* bd = GetBackendData();
				bd.InitInfo.SrvDescriptorFreeFn(&bd.InitInfo, backend_tex.FontSrvCpuDescHandle, backend_tex.FontSrvGpuDescHandle);
				Safty.DELETE!(backend_tex.TextureResource);
				backend_tex.FontSrvCpuDescHandle.Ptr = 0;
				backend_tex.FontSrvGpuDescHandle.Ptr = 0;
				ImGui.MemFree(backend_tex);

				tex.SetTexID(0);
				tex.BackendUserData = null;
			}
			tex.SetStatus(ImGui.TextureStatus.Destroyed);
		}

		public static int32 CreateDeviceObjects()
		{
			ImGuiDX12Data* bd = GetBackendData();
			if(bd == null || bd.Device == null)
				return 1;
			if(bd.PipelineState != null)
				InvalidateDeviceObjects();

			GraphicsFactory Factory = new GraphicsFactory();
			int32 hr = Factory.Create();
			if(hr > 0)
				return hr;

			bool allow_tearing = false;
			bd.Factory = Factory;
			bd.Factory.CheckFeatureSupport(Feature.PRESENT_ALLOW_TEARING, &allow_tearing, sizeof(bool));
			bd.TearingSupport = allow_tearing;

			// Create the root signature
			DescriptorRange descRange = DescriptorRange
			{
				RangeType = DescriptorRangeType.SRV,
				NumDescriptors = 1,
				BaseShaderRegister = 0,
				RegisterSpace = 0,
				OffsetInDescriptorsFromTableStart = 0
			};

			RootParameter[] param = scope RootParameter[2](?);

			param[0].ParameterType = RootParameterType._32BIT_CONSTANTS;
			param[0].Unio.Constants.ShaderRegister = 0;
			param[0].Unio.Constants.RegisterSpace = 0;
			param[0].Unio.Constants.Num32BitValues = 16;
			param[0].ShaderVisibility = ShaderVisibility.VERTEX;

			param[1].ParameterType = RootParameterType.DESCRIPTOR_TABLE;
			param[1].Unio.DescriptorTable.NumDescriptorRanges = 1;
			param[1].Unio.DescriptorTable.pDescriptorRanges = &descRange;
			param[1].ShaderVisibility = ShaderVisibility.FRAGMENT;

			StaticSamplerDesc[] staticSampler = scope StaticSamplerDesc[1](?);
			staticSampler[0].Filter = SamplerFilter.MIN_MAG_MIP_LINEAR;
			staticSampler[0].AddressU = TextureAddressMode.CLAMP;
			staticSampler[0].AddressV = TextureAddressMode.CLAMP;
			staticSampler[0].AddressW = TextureAddressMode.CLAMP;
			staticSampler[0].MipLODBias = 0.0f;
			staticSampler[0].MaxAnisotropy = 0;
			staticSampler[0].ComparisonFunc = ComparisonFunc.ALWAYS;
			staticSampler[0].BorderColor = StaticBorderColor.TRANSPARENT_BLACK;
			staticSampler[0].MinLOD = 0.0f;
			staticSampler[0].MaxLOD = float.MaxValue;
			staticSampler[0].ShaderRegister = 0;
			staticSampler[0].RegisterSpace = 0;
			staticSampler[0].ShaderVisibility = ShaderVisibility.FRAGMENT;

			RootSignatureDesc desc = RootSignatureDesc
			{
				NumParameters = uint32(param.Count),
				pParameters = param.Ptr,
				NumStaticSamplers = 1,
				pStaticSamplers = staticSampler.Ptr,
				Flags =
					RootSignatureFlags.ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT |
					RootSignatureFlags.DENY_HULL_SHADER_ROOT_ACCESS |
					RootSignatureFlags.DENY_DOMAIN_SHADER_ROOT_ACCESS |
					RootSignatureFlags.DENY_GEOMETRY_SHADER_ROOT_ACCESS
			};

			/*(hr, )bd.pDevice.CreateRootSignature(ref desc);*/
			return 0;
		}

		public static void InitMultiViewportSupport()
		{
			ImGui.PlatformIO* platform_io = ImGui.GetPlatformIO();
		}

		public static void InitDX12(ImGuiDX12InitInfo* pInitInfo)
		{
			ImGui.IO* io = ImGui.GetIO();
			ImGuiDX12Data* bd = new ImGuiDX12Data();
			bd.InitInfo = *pInitInfo;
			bd.Device = pInitInfo.Device;
			bd.CommandQueue = pInitInfo.CommandQueue;
			bd.RTVFormat = pInitInfo.RTVFormat;
			bd.DSVFormat = pInitInfo.DSVFormat;
			bd.NumFramesInFlight = uint32(pInitInfo.NumFramesInFlight);
			bd.SrvDescHeap = pInitInfo.SrvDescriptorHeap;

			bd.TearingSupport = false;

			io.BackendRendererUserData = bd;
			io.BackendRendererName = "KairosEngine_DX12";
			io.BackendFlags |= ImGui.BackendFlags.RendererHasVtxOffset;
			io.BackendFlags |= ImGui.BackendFlags.RendererHasTextures;
			io.BackendFlags |= ImGui.BackendFlags.RendererHasViewports;

			if((io.ConfigFlags & ImGui.ConfigFlags.ViewportsEnable) != 0)
				ImGuiDX12.InitMultiViewportSupport();

			ImGui.Viewport* main_viewport = ImGui.GetMainViewport();
			main_viewport.RendererUserData = System.Internal.UnsafeCastToPtr(new ImGuiDX12ViewPortData(bd.NumFramesInFlight));
		}

		public static int32 NewFrame()
		{
			ImGuiDX12Data* bd = GetBackendData();
			if(bd == null)
				return int32(ErrorCodes.BackendDataIsNull);
			if(bd.PipelineState == null)
				return CreateDeviceObjects();

			return int32(ErrorCodes.ImGUI_Success);
		}
	}
}