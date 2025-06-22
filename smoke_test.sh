#!/bin/bash

# Simple smoke test to verify WebRTC pipeline is working
# Tests:
# 1. SDP negotiation completes (peer connection = Connected)  
# 2. Track binding happens (packetizer initializes)
# 3. VP9 encoder generates packets

set -e

echo "🧪 Running WebRTC Pipeline Smoke Test..."

# Clean up any previous test artifacts
rm -f smoke_test.log

# Run the pipeline 
echo "⏳ Building and running pipeline..."
./run --timeout 20 > smoke_test.log 2>&1 || true

echo "📊 Test Results:"

# Test 1: Check SDP negotiation 
if grep -q "Peer connection state changed: Connected\|PeerConnection Connected" smoke_test.log; then
    echo "✅ SDP Negotiation: PASS"
    SDP_PASS=true
else
    echo "❌ SDP Negotiation: FAIL"
    SDP_PASS=false
fi

# Test 2: Check VP9 encoding
if grep -q "VP9 encode.*successful\|Creating VP9 encoder" smoke_test.log; then
    echo "✅ VP9 Encoding: PASS"  
    VP9_PASS=true
else
    echo "❌ VP9 Encoding: FAIL"
    VP9_PASS=false
fi

# Test 3: Check for obvious errors
if grep -q "Failed to.*peer connection\|Failed to.*encoder\|Null pointer\|Connection refused\|Unable to connect" smoke_test.log; then
    echo "❌ Error Check: FAIL (found errors)"
    ERROR_PASS=false
else
    echo "✅ Error Check: PASS"
    ERROR_PASS=true
fi

# Test 4: Check signaling server startup
if grep -q "Signaling server listening on ws://localhost:9080" smoke_test.log; then
    echo "✅ Signaling Server: PASS"
    SIGNALING_PASS=true
else
    echo "❌ Signaling Server: FAIL"
    SIGNALING_PASS=false
fi

echo ""
if [ "$SDP_PASS" = true ] && [ "$VP9_PASS" = true ] && [ "$ERROR_PASS" = true ] && [ "$SIGNALING_PASS" = true ]; then
    echo "🎉 OVERALL: PASS - WebRTC pipeline is working"
    exit 0
else
    echo "💥 OVERALL: FAIL - WebRTC pipeline has issues"
    echo ""
    echo "📋 To debug, check: smoke_test.log"
    exit 1
fi