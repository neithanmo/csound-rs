#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![no_std]


#[doc(inline)]
pub use selected_bindings::*;

/// A selection of the ffi bindings intended to be used directly.
///
/// The full list of bindings is under the [ffi_bindgen] submodule.
///
/// The current module publicly re-exports bindgen generated structs, functions,
/// and constants, for their direct usage.
mod selected_bindings {

    /// Rust FFI bindings, automatically generated with bindgen.
    // [clippy & bindgen](https://github.com/rust-lang/rust-bindgen/issues/1470)
    #[allow(clippy::all)]
    pub mod ffi_bindgen {
        include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
    }


    #[doc(inline)]
    pub use ffi_bindgen::{
        // functions
        csoundAddSpinSample,
        csoundAppendOpcode,
        csoundCfgErrorCodeToString,
        csoundCleanup,
        csoundClearSpin,
        csoundCloseLibrary,
        csoundCompile,
        csoundCompileArgs,
        csoundCompileCsd,
        csoundCompileCsdText,
        csoundCompileOrc,
        csoundCompileOrcAsync,
        csoundCompileTree,
        csoundCompileTreeAsync,
        csoundCondSignal,
        csoundCondWait,
        csoundCreate,
        csoundCreateBarrier,
        csoundCreateCircularBuffer,
        csoundCreateCondVar,
        csoundCreateConfigurationVariable,
        csoundCreateGlobalVariable,
        csoundCreateMessageBuffer,
        csoundCreateMutex,
        csoundCreateThread,
        csoundCreateThreadLock,
        csoundDeleteCfgVarList,
        csoundDeleteChannelList,
        csoundDeleteConfigurationVariable,
        csoundDeleteTree,
        csoundDeleteUtilityList,
        csoundDestroy,
        csoundDestroyBarrier,
        csoundDestroyCircularBuffer,
        csoundDestroyCondVar,
        csoundDestroyGlobalVariable,
        csoundDestroyMessageBuffer,
        csoundDestroyMutex,
        csoundDestroyThreadLock,
        csoundDisposeOpcodeList,
        csoundEvalCode,
        csoundFlushCircularBuffer,
        csoundGet0dBFS,
        csoundGetA4,
        csoundGetAPIVersion,
        csoundGetAudioChannel,
        csoundGetAudioDevList,
        csoundGetCPUTime,
        csoundGetChannelDatasize,
        csoundGetChannelLock,
        csoundGetChannelPtr,
        csoundGetControlChannel,
        csoundGetControlChannelHints,
        csoundGetCurrentThreadId,
        csoundGetCurrentTimeSamples,
        csoundGetDebug,
        csoundGetEnv,
        csoundGetFirstMessage,
        csoundGetFirstMessageAttr,
        csoundGetHostData,
        csoundGetInputBuffer,
        csoundGetInputBufferSize,
        csoundGetInputName,
        csoundGetKr,
        csoundGetKsmps,
        csoundGetLibrarySymbol,
        csoundGetMIDIDevList,
        csoundGetMessageCnt,
        csoundGetMessageLevel,
        csoundGetModule,
        csoundGetNamedGEN,
        csoundGetNamedGens,
        csoundGetNchnls,
        csoundGetNchnlsInput,
        csoundGetOutputBuffer,
        csoundGetOutputBufferSize,
        csoundGetOutputFormat,
        csoundGetOutputName,
        csoundGetParams,
        csoundGetPvsChannel,
        csoundGetRandomSeedFromTime,
        csoundGetRealTime,
        csoundGetRtPlayUserData,
        csoundGetRtRecordUserData,
        csoundGetScoreOffsetSeconds,
        csoundGetScoreTime,
        csoundGetSizeOfMYFLT,
        csoundGetSpin,
        csoundGetSpout,
        csoundGetSpoutSample,
        csoundGetSr,
        csoundGetStringChannel,
        csoundGetTable,
        csoundGetTableArgs,
        csoundGetUtilityDescription,
        csoundGetVersion,
        csoundInitTimerStruct,
        csoundInitialize,
        csoundInitializeCscore,
        csoundInputMessage,
        csoundInputMessageAsync,
        csoundIsNamedGEN,
        csoundIsScorePending,
        csoundJoinThread,
        csoundKeyPress,
        csoundKillInstance,
        csoundListChannels,
        csoundListConfigurationVariables,
        csoundListUtilities,
        csoundLoadPlugins,
        csoundLocalizeString,
        csoundLockMutex,
        csoundLockMutexNoWait,
        csoundMessage,
        csoundMessageS,
        csoundMessageV,
        csoundNewOpcodeList,
        csoundNotifyThreadLock,
        csoundOpenLibrary,
        csoundParseConfigurationVariable,
        csoundParseOrc,
        csoundPeekCircularBuffer,
        csoundPerform,
        csoundPerformBuffer,
        csoundPerformKsmps,
        csoundPopFirstMessage,
        csoundQueryConfigurationVariable,
        csoundQueryGlobalVariable,
        csoundQueryGlobalVariableNoCheck,
        csoundRand31,
        csoundRandMT,
        csoundReadCircularBuffer,
        csoundReadScore,
        csoundReadScoreAsync,
        csoundRegisterKeyboardCallback,
        csoundRegisterSenseEventCallback,
        csoundRemoveKeyboardCallback,
        csoundReset,
        csoundRewindScore,
        csoundRunCommand,
        csoundRunUtility,
        csoundScoreEvent,
        csoundScoreEventAbsolute,
        csoundScoreEventAbsoluteAsync,
        csoundScoreEventAsync,
        csoundScoreExtract,
        csoundScoreSort,
        csoundSeedRandMT,
        csoundSetAudioChannel,
        csoundSetAudioDeviceListCallback,
        csoundSetConfigurationVariable,
        csoundSetControlChannel,
        csoundSetControlChannelHints,
        csoundSetCscoreCallback,
        csoundSetDebug,
        csoundSetDefaultMessageCallback,
        csoundSetDrawGraphCallback,
        csoundSetExitGraphCallback,
        csoundSetExternalMidiErrorStringCallback,
        csoundSetExternalMidiInCloseCallback,
        csoundSetExternalMidiInOpenCallback,
        csoundSetExternalMidiOutCloseCallback,
        csoundSetExternalMidiOutOpenCallback,
        csoundSetExternalMidiReadCallback,
        csoundSetExternalMidiWriteCallback,
        csoundSetFileOpenCallback,
        csoundSetGlobalEnv,
        csoundSetHostData,
        csoundSetHostImplementedAudioIO,
        csoundSetHostImplementedMIDIIO,
        csoundSetInput,
        csoundSetInputChannelCallback,
        csoundSetIsGraphable,
        csoundSetKillGraphCallback,
        csoundSetLanguage,
        csoundSetMIDIDeviceListCallback,
        csoundSetMIDIFileInput,
        csoundSetMIDIFileOutput,
        csoundSetMIDIInput,
        csoundSetMIDIModule,
        csoundSetMIDIOutput,
        csoundSetMakeGraphCallback,
        csoundSetMessageCallback,
        csoundSetMessageLevel,
        csoundSetMessageStringCallback,
        csoundSetOpcodedir,
        csoundSetOption,
        csoundSetOutput,
        csoundSetOutputChannelCallback,
        csoundSetParams,
        csoundSetPlayopenCallback,
        csoundSetPvsChannel,
        csoundSetRTAudioModule,
        csoundSetRecopenCallback,
        csoundSetRtcloseCallback,
        csoundSetRtplayCallback,
        csoundSetRtrecordCallback,
        csoundSetScoreOffsetSeconds,
        csoundSetScorePending,
        csoundSetSpinSample,
        csoundSetStringChannel,
        csoundSetYieldCallback,
        csoundSleep,
        csoundSpinLock,
        csoundSpinLockInit,
        csoundSpinTryLock,
        csoundSpinUnLock,
        csoundStart,
        csoundStop,
        csoundStopUDPConsole,
        csoundSystemSr,
        csoundTableCopyIn,
        csoundTableCopyInAsync,
        csoundTableCopyOut,
        csoundTableCopyOutAsync,
        csoundTableGet,
        csoundTableLength,
        csoundTableSet,
        csoundUDPConsole,
        csoundUDPServerClose,
        csoundUDPServerStart,
        csoundUDPServerStatus,
        csoundUnlockMutex,
        csoundWaitBarrier,
        csoundWaitThreadLock,
        csoundWaitThreadLockNoTimeout,
        csoundWriteCircularBuffer,

        // constants
        CSOUNDCFG_BOOLEAN,
        CSOUNDCFG_DOUBLE,
        CSOUNDCFG_FLOAT,
        CSOUNDCFG_INTEGER,
        CSOUNDCFG_INVALID_BOOLEAN,
        CSOUNDCFG_INVALID_FLAG,
        CSOUNDCFG_INVALID_NAME,
        CSOUNDCFG_INVALID_TYPE,
        CSOUNDCFG_LASTERROR,
        CSOUNDCFG_MEMORY,
        CSOUNDCFG_MYFLT,
        CSOUNDCFG_NOT_POWOFTWO,
        CSOUNDCFG_NULL_POINTER,
        CSOUNDCFG_POWOFTWO,
        CSOUNDCFG_STRING,
        CSOUNDCFG_STRING_LENGTH,
        CSOUNDCFG_SUCCESS,
        CSOUNDCFG_TOO_HIGH,
        CSOUNDCFG_TOO_LOW,
        CSOUNDINIT_NO_ATEXIT,
        CSOUNDINIT_NO_SIGNAL_HANDLER,
        CSOUNDMSG_BG_BLACK,
        CSOUNDMSG_BG_BLUE,
        CSOUNDMSG_BG_COLOR_MASK,
        CSOUNDMSG_BG_CYAN,
        CSOUNDMSG_BG_GREEN,
        CSOUNDMSG_BG_GREY,
        CSOUNDMSG_BG_MAGENTA,
        CSOUNDMSG_BG_ORANGE,
        CSOUNDMSG_BG_RED,
        CSOUNDMSG_DEFAULT,
        CSOUNDMSG_ERROR,
        CSOUNDMSG_FG_ATTR_MASK,
        CSOUNDMSG_FG_BLACK,
        CSOUNDMSG_FG_BLUE,
        CSOUNDMSG_FG_BOLD,
        CSOUNDMSG_FG_COLOR_MASK,
        CSOUNDMSG_FG_CYAN,
        CSOUNDMSG_FG_GREEN,
        CSOUNDMSG_FG_MAGENTA,
        CSOUNDMSG_FG_RED,
        CSOUNDMSG_FG_UNDERLINE,
        CSOUNDMSG_FG_WHITE,
        CSOUNDMSG_FG_YELLOW,
        CSOUNDMSG_ORCH,
        CSOUNDMSG_REALTIME,
        CSOUNDMSG_STDOUT,
        CSOUNDMSG_TYPE_MASK,
        CSOUNDMSG_WARNING,
        CSOUND_CALLBACK_KBD_EVENT,
        CSOUND_CALLBACK_KBD_TEXT,
        CSOUND_EXITJMP_SUCCESS,
        CS_APISUBVER,
        CS_APIVERSION,
        CS_PACKAGE_NAME,
        CS_PACKAGE_STRING,
        CS_PACKAGE_TARNAME,
        CS_PACKAGE_VERSION,
        CS_PATCHLEVEL,
        CS_SUBVER,
        CS_VERSION,

        // types
        CSOUND,
        CsoundRandMTState,
        PVSDATEXT,
        RTCLOCK,
        WINDAT,
        XYINDAT,
        channelCallback_t,
        controlChannelHints_t,
        controlChannelInfo_t,
        csCfgVariableBoolean_t,
        csCfgVariableDouble_t,
        csCfgVariableFloat_t,
        csCfgVariableHead_t,
        csCfgVariableInt_t,
        csCfgVariableMYFLT_t,
        csCfgVariableString_t,
        csCfgVariable_t,

        // structs (commented out the ones that already have a type alias)
        // CSOUND_,
        CSOUND_PARAMS,
        CS_AUDIODEVICE,
        CS_MIDIDEVICE,
        // CsoundRandMTState_,
        ORCTOKEN,
        // RTCLOCK_S,
        TREE,
        __va_list_tag,
        // controlChannelHints_s,
        // controlChannelInfo_s,
        // csCfgVariableBoolean_s,
        // csCfgVariableDouble_s,
        // csCfgVariableFloat_s,
        // csCfgVariableHead_s,
        // csCfgVariableInt_s,
        // csCfgVariableMYFLT_s,
        // csCfgVariableString_s,
        csRtAudioParams,
        opcodeListEntry,
        pvsdat_ext,
        // windat_,
        // xyindat_,

        // modules
        CSOUND_FILETYPES,
        CSOUND_STATUS,
        controlChannelBehavior,
        controlChannelType,
        cslanguage_t,
        idtype_t,
    };
}
