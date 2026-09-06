# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Serve bounded ASP Python Graphs requests over a private gRPC stream."""

from __future__ import annotations

import asyncio
import json
from collections.abc import AsyncIterator, Mapping
from pathlib import Path

import grpc

from .service_protocol import ServiceProtocolError, unavailable_receipt
from .service_session import AspPythonGraphsSession


SERVICE_NAME = "asp.python.graphs.AspPythonGraphs"
SESSION_METHOD = f"/{SERVICE_NAME}/Session"
DEFAULT_MAX_IN_FLIGHT = 32


def encode_envelope(message: Mapping[str, object]) -> bytes:
    payload = json.dumps(message, sort_keys=True, separators=(",", ":")).encode()
    return b"\x0a" + _encode_varint(len(payload)) + payload


def decode_envelope(encoded: bytes) -> dict[str, object]:
    if not encoded or encoded[0] != 0x0A:
        raise ValueError("ASP Python Graphs envelope requires protobuf bytes field 1")
    length, offset = _decode_varint(encoded, 1)
    payload = encoded[offset : offset + length]
    if len(payload) != length or offset + length != len(encoded):
        raise ValueError("ASP Python Graphs envelope length mismatch")
    decoded = json.loads(payload)
    if not isinstance(decoded, dict):
        raise ValueError("ASP Python Graphs JSON payload must be an object")
    return decoded


class AspPythonGraphsGrpcHandler(grpc.GenericRpcHandler):
    def __init__(self, max_in_flight: int = DEFAULT_MAX_IN_FLIGHT) -> None:
        if max_in_flight < 1:
            raise ValueError("max_in_flight must be positive")
        self._max_in_flight = max_in_flight

    def service(self, handler_call_details: grpc.HandlerCallDetails):  # type: ignore[no-untyped-def]
        if handler_call_details.method != SESSION_METHOD:
            return None
        return grpc.stream_stream_rpc_method_handler(
            self.session,
            request_deserializer=decode_envelope,
            response_serializer=encode_envelope,
        )

    async def session(
        self,
        request_iterator: AsyncIterator[dict[str, object]],
        context: grpc.aio.ServicerContext,
    ) -> AsyncIterator[dict[str, object]]:
        session = AspPythonGraphsSession()
        capacity = asyncio.Semaphore(self._max_in_flight)
        outbound: asyncio.Queue[dict[str, object] | None] = asyncio.Queue(
            self._max_in_flight
        )
        tasks: set[asyncio.Task[None]] = set()
        active_evaluates = 0

        async def evaluate(message: dict[str, object]) -> None:
            nonlocal active_evaluates
            try:
                receipt = await asyncio.to_thread(session.handle_admitted, message)
            except ServiceProtocolError as error:
                receipt = unavailable_receipt(message, error.code, str(error))
            except Exception as error:  # fail closed at the service boundary
                receipt = unavailable_receipt(
                    message, "algorithm-execution-failed", str(error)
                )
            try:
                # A cancellation may have won the registry race while the
                # immutable graph job was finishing.  Its terminal is owned
                # by the control path; never publish the late result.
                if message.get("messageKind") in {
                    "timeline",
                    "evaluate-resident",
                } and session.is_cancelled(str(message.get("requestId", ""))):
                    session.complete_request(str(message.get("requestId", "")))
                    return
                await outbound.put(receipt)
            finally:
                active_evaluates -= 1
                capacity.release()

        async def control(message: dict[str, object]) -> None:
            try:
                receipt = session.handle_admitted(message)
            except ServiceProtocolError as error:
                receipt = unavailable_receipt(message, error.code, str(error))
            except Exception as error:  # fail closed at the service boundary
                receipt = unavailable_receipt(
                    message, "service-protocol-failed", str(error)
                )
            await outbound.put(receipt)

        async def read_requests() -> None:
            nonlocal active_evaluates
            try:
                async for message in request_iterator:
                    kind = message.get("messageKind")
                    try:
                        session.admit_message(message, validate_service_epoch=False)
                    except ServiceProtocolError as error:
                        await outbound.put(
                            unavailable_receipt(message, error.code, str(error))
                        )
                        continue
                    if kind in {"timeline", "evaluate-resident"}:
                        if active_evaluates >= self._max_in_flight:
                            await outbound.put(
                                unavailable_receipt(
                                    message,
                                    "capacity-exhausted",
                                    "ASP Python Graphs operation capacity is saturated",
                                )
                            )
                            continue
                        await capacity.acquire()
                        active_evaluates += 1
                        try:
                            session.register_request(str(message.get("requestId", "")))
                        except ServiceProtocolError as error:
                            capacity.release()
                            active_evaluates -= 1
                            await outbound.put(
                                unavailable_receipt(message, error.code, str(error))
                            )
                            continue
                        task = asyncio.create_task(evaluate(message))
                        tasks.add(task)
                        task.add_done_callback(tasks.discard)
                    else:
                        # Lifecycle and generation mutations are actor-ordered.
                        # A release/shutdown drains already admitted evaluate
                        # jobs before mutating the generation table; cancel is
                        # intentionally immediate so it can suppress a late
                        # result from an in-flight immutable snapshot.
                        if kind in {"release-generation", "shutdown"}:
                            await asyncio.gather(*tuple(tasks), return_exceptions=True)
                        await control(message)
                if tasks:
                    await asyncio.gather(*tuple(tasks), return_exceptions=True)
            finally:
                await outbound.put(None)

        producer = asyncio.create_task(read_requests())
        try:
            while True:
                receipt = await outbound.get()
                if receipt is None:
                    break
                yield receipt
        finally:
            producer.cancel()
            for task in tuple(tasks):
                task.cancel()
            await asyncio.gather(producer, *tuple(tasks), return_exceptions=True)


async def serve(socket_path: Path, max_in_flight: int) -> None:
    server = grpc.aio.server(
        maximum_concurrent_rpcs=max_in_flight,
        options=(("grpc.so_reuseport", 0),),
    )
    server.add_generic_rpc_handlers((AspPythonGraphsGrpcHandler(max_in_flight),))
    endpoint = f"unix:{socket_path}"
    if server.add_insecure_port(endpoint) != 1:
        raise RuntimeError(
            f"failed to bind ASP Python Graphs gRPC endpoint: {endpoint}"
        )
    await server.start()
    await server.wait_for_termination()


def _encode_varint(value: int) -> bytes:
    output = bytearray()
    while value >= 0x80:
        output.append((value & 0x7F) | 0x80)
        value >>= 7
    output.append(value)
    return bytes(output)


def _decode_varint(encoded: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while offset < len(encoded) and shift <= 63:
        current = encoded[offset]
        offset += 1
        value |= (current & 0x7F) << shift
        if current & 0x80 == 0:
            return value, offset
        shift += 7
    raise ValueError("invalid protobuf varint")
