#ifndef __KAIROS_WINDOW__
#define __KAIROS_WINDOW__

#include "KairosEngineDefines.h"
#include <Windows.h>
#include <functional>

KAIROS_EXPORT_BEGIN

typedef INT64(__stdcall* WndProcCallback)(
    HWND hwnd,
    UINT msg,
    UINT64 wParam,
    INT64 lParam
    );

HWND KAIROS_API KairosInitializeWindow(HINSTANCE hInstance, int ShowWnd, int width, int height, bool fullScreen, LPCWSTR windowName, LPCWSTR windowTitle, WndProcCallback wndProc);

int KAIROS_API KairosDefWindowProcW(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam);

void KAIROS_API KairosDestroyWindow(HWND hWnd);

void KAIROS_API KairosPostQuitMessage(int nExitCode);

void KAIROS_API KairosMainLoop();

void KAIROS_API KairosInitMSG(MSG* pMsg);

int KAIROS_API KairosPeekMessage(MSG* pMsg);

int KAIROS_API KairosTranslateMessage(MSG* pMsg);

LRESULT KAIROS_API KairosDispatchMessage(MSG* pMsg);

KAIROS_EXPORT_END

#endif // __KAIROS_WINDOW__

