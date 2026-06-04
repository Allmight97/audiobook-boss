using System.Text.Json;
using System.Text.Json.Serialization;

namespace AbbAaxcleanHelper;

internal static class Protocol
{
    internal const int SchemaVersion = 1;

    internal static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) },
    };
}

internal sealed record MaterializeRequest(
    int SchemaVersion,
    string OperationId,
    MaterializeLane Lane,
    string InputPath,
    string OutputTempPath,
    MaterializeSecret Secret
);

internal enum MaterializeLane
{
    Aax,
    Aaxc,
}

internal sealed record MaterializeSecret(
    string? ActivationBytesHex,
    string? KeyHex,
    string? IvHex
);

internal abstract record HelperMessage(string Type, string OperationId);

internal sealed record ProgressMessage(string OperationId, double Fraction)
    : HelperMessage("progress", OperationId);

internal sealed record ResultMessage(string OperationId, long BytesWritten)
    : HelperMessage("result", OperationId);

internal sealed record ErrorMessage(string OperationId, string Category, string Message)
    : HelperMessage("error", OperationId);
