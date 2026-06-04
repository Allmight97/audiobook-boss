namespace AbbAaxcleanHelper;

internal static class Validation
{
    internal static string Validate(MaterializeRequest? request)
    {
        if (request is null) return "Request JSON was missing or invalid.";
        if (request.SchemaVersion != Protocol.SchemaVersion) return "Unsupported request schema.";
        if (string.IsNullOrWhiteSpace(request.OperationId)) return "Operation id is required.";
        if (string.IsNullOrWhiteSpace(request.InputPath)) return "Input path is required.";
        if (string.IsNullOrWhiteSpace(request.OutputTempPath)) return "Output path is required.";
        if (request.Secret is null) return "Secret payload is required.";

        return request.Lane switch
        {
            MaterializeLane.Aax when !IsHexLength(request.Secret.ActivationBytesHex, 8)
                => "AAX activation bytes are required.",
            MaterializeLane.Aax when request.Secret.KeyHex is not null || request.Secret.IvHex is not null
                => "AAX request used AAXC secret fields.",
            MaterializeLane.Aaxc when !IsHexLength(request.Secret.KeyHex, 32)
                => "AAXC key is required.",
            MaterializeLane.Aaxc when !IsHexLength(request.Secret.IvHex, 32)
                => "AAXC IV is required.",
            MaterializeLane.Aaxc when request.Secret.ActivationBytesHex is not null
                => "AAXC request used AAX secret fields.",
            _ => string.Empty,
        };
    }

    private static bool IsHexLength(string? value, int length)
    {
        return value is { Length: var actualLength }
            && actualLength == length
            && value.All(Uri.IsHexDigit);
    }
}
