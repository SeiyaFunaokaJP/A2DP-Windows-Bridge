/*
 * Application audio capture helpers - Implementation
 *
 * SPDX-License-Identifier: MIT
 */

#include "app_audio.h"

#include <mmdeviceapi.h>
#include <audiopolicy.h>
#include <tlhelp32.h>
#include <algorithm>
#include <map>

namespace {

std::string to_utf8(const std::wstring &w) {
    if (w.empty()) return {};
    int n = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), (int)w.size(), nullptr, 0, nullptr, nullptr);
    std::string s(n > 0 ? n : 0, '\0');
    if (n > 0) WideCharToMultiByte(CP_UTF8, 0, w.c_str(), (int)w.size(), &s[0], n, nullptr, nullptr);
    return s;
}

std::wstring lower(std::wstring s) {
    for (auto &c : s) c = (wchar_t)towlower(c);
    return s;
}

std::wstring file_name(const std::wstring &path) {
    size_t pos = path.find_last_of(L"\\/");
    return pos == std::wstring::npos ? path : path.substr(pos + 1);
}

std::wstring process_image_path(DWORD pid) {
    HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!h) return {};
    wchar_t buf[MAX_PATH];
    DWORD len = MAX_PATH;
    std::wstring path;
    if (QueryFullProcessImageNameW(h, 0, buf, &len)) path.assign(buf, len);
    CloseHandle(h);
    return path;
}

/* FileDescription from the version resource, e.g. "Music Player" */
std::wstring file_description(const std::wstring &path) {
    DWORD dummy = 0;
    DWORD size = GetFileVersionInfoSizeW(path.c_str(), &dummy);
    if (size == 0) return {};
    std::vector<uint8_t> data(size);
    if (!GetFileVersionInfoW(path.c_str(), 0, size, data.data())) return {};

    struct LangCodePage { WORD lang, cp; } *tr = nullptr;
    UINT tr_len = 0;
    if (!VerQueryValueW(data.data(), L"\\VarFileInfo\\Translation", (void **)&tr, &tr_len) ||
        tr_len < sizeof(LangCodePage))
        return {};
    wchar_t key[64];
    swprintf(key, 64, L"\\StringFileInfo\\%04x%04x\\FileDescription", tr[0].lang, tr[0].cp);
    wchar_t *desc = nullptr;
    UINT desc_len = 0;
    if (!VerQueryValueW(data.data(), key, (void **)&desc, &desc_len) || desc_len == 0) return {};
    return std::wstring(desc);
}

/* Process IDs of the app audio sessions on all active output devices.
 * Works on any thread: joins or creates the thread's COM apartment. */
std::vector<DWORD> session_pids() {
    std::vector<DWORD> pids;
    HRESULT co = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    IMMDeviceEnumerator *enumerator = nullptr;
    HRESULT hr = CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
                                  __uuidof(IMMDeviceEnumerator), (void **)&enumerator);
    if (FAILED(hr)) {
        if (SUCCEEDED(co)) CoUninitialize();
        return pids;
    }

    IMMDeviceCollection *devices = nullptr;
    if (SUCCEEDED(enumerator->EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE, &devices))) {
        UINT count = 0;
        devices->GetCount(&count);
        for (UINT i = 0; i < count; i++) {
            IMMDevice *device = nullptr;
            if (FAILED(devices->Item(i, &device))) continue;
            IAudioSessionManager2 *mgr = nullptr;
            IAudioSessionEnumerator *sessions = nullptr;
            if (SUCCEEDED(device->Activate(__uuidof(IAudioSessionManager2), CLSCTX_ALL, nullptr,
                                           (void **)&mgr)) &&
                SUCCEEDED(mgr->GetSessionEnumerator(&sessions))) {
                int n = 0;
                sessions->GetCount(&n);
                for (int s = 0; s < n; s++) {
                    IAudioSessionControl *ctl = nullptr;
                    IAudioSessionControl2 *ctl2 = nullptr;
                    if (FAILED(sessions->GetSession(s, &ctl))) continue;
                    DWORD pid = 0;
                    if (SUCCEEDED(ctl->QueryInterface(__uuidof(IAudioSessionControl2), (void **)&ctl2))) {
                        /* S_OK = the system sounds session, not an app */
                        if (ctl2->IsSystemSoundsSession() != S_OK) ctl2->GetProcessId(&pid);
                        ctl2->Release();
                    }
                    ctl->Release();
                    if (pid != 0) pids.push_back(pid);
                }
            }
            if (sessions) sessions->Release();
            if (mgr) mgr->Release();
            device->Release();
        }
        devices->Release();
    }
    enumerator->Release();
    if (SUCCEEDED(co)) CoUninitialize();
    return pids;
}

} /* namespace */

namespace app_audio {

uint32_t windows_build() {
    using RtlGetVersionFn = LONG(WINAPI *)(PRTL_OSVERSIONINFOW);
    HMODULE ntdll = GetModuleHandleW(L"ntdll.dll");
    auto fn = ntdll ? reinterpret_cast<RtlGetVersionFn>(GetProcAddress(ntdll, "RtlGetVersion")) : nullptr;
    if (!fn) return 0;
    RTL_OSVERSIONINFOW vi = {};
    vi.dwOSVersionInfoSize = sizeof(vi);
    if (fn(&vi) != 0) return 0;
    return vi.dwBuildNumber;
}

bool capture_supported() {
    return windows_build() >= APP_CAPTURE_MIN_BUILD;
}

std::vector<AudioAppInfo> list_audio_apps() {
    std::map<std::wstring, AudioAppInfo> by_exe;  /* key: lowercase exe name */
    DWORD self = GetCurrentProcessId();
    for (DWORD pid : session_pids()) {
        if (pid == self) continue;
        std::wstring path = process_image_path(pid);
        if (path.empty()) continue;
        std::wstring exe = file_name(path);
        std::wstring key = lower(exe);
        if (by_exe.count(key)) continue;
        std::wstring desc = file_description(path);
        AudioAppInfo info;
        info.exe_name = to_utf8(exe);
        info.display_name = desc.empty() ? info.exe_name : to_utf8(desc) + " (" + info.exe_name + ")";
        by_exe[key] = info;
    }

    std::vector<AudioAppInfo> out;
    for (auto &kv : by_exe) out.push_back(kv.second);
    std::sort(out.begin(), out.end(), [](const AudioAppInfo &a, const AudioAppInfo &b) {
        return _stricmp(a.display_name.c_str(), b.display_name.c_str()) < 0;
    });
    return out;
}

DWORD find_process(const std::string &exe_name) {
    if (exe_name.empty()) return 0;
    int n = MultiByteToWideChar(CP_UTF8, 0, exe_name.c_str(), (int)exe_name.size(), nullptr, 0);
    std::wstring want(n > 0 ? n : 0, L'\0');
    if (n > 0) MultiByteToWideChar(CP_UTF8, 0, exe_name.c_str(), (int)exe_name.size(), &want[0], n);
    want = lower(want);

    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE) return 0;
    std::map<DWORD, DWORD> matches;  /* pid -> parent pid */
    PROCESSENTRY32W pe = {};
    pe.dwSize = sizeof(pe);
    for (BOOL ok = Process32FirstW(snap, &pe); ok; ok = Process32NextW(snap, &pe)) {
        if (lower(pe.szExeFile) == want) matches[pe.th32ProcessID] = pe.th32ParentProcessID;
    }
    CloseHandle(snap);

    if (matches.empty()) return 0;

    /* The top of a tree: walk up while the parent is another match */
    auto top = [&matches](DWORD pid) {
        for (int guard = 0; guard < 64; guard++) {
            auto it = matches.find(pid);
            if (it == matches.end() || !matches.count(it->second) || it->second == pid) break;
            pid = it->second;
        }
        return pid;
    };
    /* Prefer the instance that is playing audio (several may be running) */
    for (DWORD pid : session_pids())
        if (matches.count(pid)) return top(pid);
    for (auto &m : matches)
        if (!matches.count(m.second)) return m.first;
    return matches.begin()->first;
}

} /* namespace app_audio */
