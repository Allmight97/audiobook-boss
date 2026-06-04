using System.Text.Json;
using AAXClean;

namespace AbbAaxcleanHelper;

internal static class Program
{
    private const int SuccessExitCode = 0;
    private const int InvalidRequestExitCode = 2;
    private const int MaterializationFailedExitCode = 3;

    private static async Task<int> Main()
    {
        string operationId = "unknown";
        try
        {
            var stdin = await Console.In.ReadToEndAsync();
            var request = JsonSerializer.Deserialize<MaterializeRequest>(stdin, Protocol.JsonOptions);
            var validationError = Validation.Validate(request);
            if (validationError.Length > 0 || request is null)
            {
                await WriteAsync(new ErrorMessage(operationId, "invalid_request", validationError));
                return InvalidRequestExitCode;
            }

            operationId = request.OperationId;
            await MaterializeAsync(request);
            var bytesWritten = new FileInfo(request.OutputTempPath).Length;
            await WriteAsync(new ResultMessage(operationId, bytesWritten));
            return SuccessExitCode;
        }
        catch (Exception)
        {
            await WriteAsync(new ErrorMessage(
                operationId,
                "materialization_failed",
                "AAXClean helper failed during materialization."
            ));
            return MaterializationFailedExitCode;
        }
    }

    private static async Task MaterializeAsync(MaterializeRequest request)
    {
        await using var input = File.Open(request.InputPath, FileMode.Open, FileAccess.Read, FileShare.Read);
        await using var output = File.Open(request.OutputTempPath, FileMode.CreateNew, FileAccess.ReadWrite, FileShare.None);
        var aaxFile = new AaxFile(input);

        switch (request.Lane)
        {
            case MaterializeLane.Aax:
                aaxFile.SetDecryptionKey(request.Secret.ActivationBytesHex!);
                break;
            case MaterializeLane.Aaxc:
                aaxFile.SetDecryptionKey(request.Secret.KeyHex!, request.Secret.IvHex!);
                break;
            default:
                throw new InvalidOperationException("Unsupported materialization lane.");
        }

        var operation = aaxFile.ConvertToMp4aAsync(output);
        operation.ConversionProgressUpdate += async (_, args) =>
            await WriteAsync(new ProgressMessage(request.OperationId, args.FractionCompleted));
        operation.Start();
        await operation.OperationTask;
    }

    private static async Task WriteAsync(HelperMessage message)
    {
        var line = JsonSerializer.Serialize(message, message.GetType(), Protocol.JsonOptions);
        await Console.Out.WriteLineAsync(line);
        await Console.Out.FlushAsync();
    }
}
