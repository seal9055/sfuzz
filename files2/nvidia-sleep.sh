#!/bin/bash

if [ ! -f /proc/driver/nvidia/suspend ]; then
    exit 0
fi

RUN_DIR="/var/run/nvidia-sleep"
XORG_VT_FILE="${RUN_DIR}"/Xorg.vt_number

PATH="/bin:/usr/bin"

case "$1" in
    suspend|hibernate)
        mkdir -p "${RUN_DIR}"
        fgconsole > "${XORG_VT_FILE}"
        chvt 63
        if [[ $? -ne 0 ]]; then
            exit $?
        fi
        echo "$1" > /proc/driver/nvidia/suspend
        exit $?
        ;;
    resume)
        echo "$1" > /proc/driver/nvidia/suspend 
        #
        # Check if Xorg was determined to be running at the time
        # of suspend, and whether its VT was recorded.  If so,
        # attempt to switch back to this VT.
        #
        if [[ -f "${XORG_VT_FILE}" ]]; then
            XORG_PID=$(cat "${XORG_VT_FILE}")
            rm "${XORG_VT_FILE}"
            chvt "${XORG_PID}"
        fi
        exit 0
        ;;
    *)
        exit 1
esac
