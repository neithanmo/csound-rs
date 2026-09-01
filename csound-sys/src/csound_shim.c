#include "csdl.h"

/*
 * Host API ownership shim for csoundGetControlChannelHints().
 *
 * csound.h documents that csoundGetControlChannelHints() allocates the
 * attributes member and requires the host to free it, but the public host API
 * provides no matching deallocation function. The implementation allocates
 * attributes through csound->Malloc(), whose allocations are tracked by the
 * specific CSOUND instance. Consequently, libc free() is invalid and using a
 * different CSOUND instance's allocator can corrupt allocator state.
 *
 * CSOUND::Free is available through the plugin interface exposed by csdl.h,
 * even though the underlying csoundFree() function is not part of the public
 * host API or dynamically exported. This purpose-specific shim supplies the
 * missing ownership operation until Csound provides an equivalent public API.
 * It must receive the same CSOUND instance that produced the hints. Clearing
 * attributes after release prevents accidental reuse or double-free.
 *
 * This does not apply to hints embedded in csoundListChannels() results. Those
 * attribute pointers are borrowed engine data; only the outer channel list is
 * released, using csoundDeleteChannelList().
 */
void csoundFreeControlChannelHints(CSOUND *csound,
                                   controlChannelHints_t *hints) {
    if (csound != NULL && hints != NULL && hints->attributes != NULL) {
        csound->Free(csound, hints->attributes);
        hints->attributes = NULL;
    }
}
