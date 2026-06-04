using System.Text.Json;
using AAXClean;

namespace AbbAaxcleanHelper;

internal static class Program
{
    private static readonly SemaphoreSlim WriteSemaphore = new(1, 1);

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
        catch (HelperFailure failure)
        {
            await WriteAsync(new ErrorMessage(
                operationId,
                failure.Category,
                failure.SafeMessage
            ));
            return MaterializationFailedExitCode;
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
        await using var input = OpenInput(request.InputPath);
        await using var output = OpenOutput(request.OutputTempPath);
        var aaxFile = ParseAax(input);

        try
        {
            switch (request.Lane)
            {
                case MaterializeLane.Aax:
                    aaxFile.SetDecryptionKey(request.Secret.ActivationBytesHex!);
                    break;
                case MaterializeLane.Aaxc:
                    aaxFile.SetDecryptionKey(request.Secret.KeyHex!, request.Secret.IvHex!);
                    break;
                default:
                    throw new HelperFailure(
                        "invalid_request",
                        "AAXClean helper received an unsupported materialization lane."
                    );
            }

            var operation = aaxFile.ConvertToMp4aAsync(output);
            operation.ConversionProgressUpdate += async (_, args) =>
                await WriteAsync(new ProgressMessage(request.OperationId, args.FractionCompleted));
            operation.Start();
            await operation.OperationTask;
        }
        catch (HelperFailure)
        {
            throw;
        }
        catch (Exception)
        {
            throw new HelperFailure(
                "conversion_failed",
                "AAXClean helper conversion failed."
            );
        }
    }

    private static FileStream OpenInput(string path)
    {
        try
        {
            return File.Open(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        }
        catch (Exception)
        {
            throw new HelperFailure(
                "input_open_failed",
                "AAXClean helper could not open the protected input."
            );
        }
    }

    private static FileStream OpenOutput(string path)
    {
        try
        {
            return File.Open(path, FileMode.CreateNew, FileAccess.ReadWrite, FileShare.None);
        }
        catch (Exception)
        {
            throw new HelperFailure(
                "output_open_failed",
                "AAXClean helper could not create the output file."
            );
        }
    }

    private static AaxFile ParseAax(Stream input)
    {
        try
        {
            return new AaxFile(input);
        }
        catch (Exception)
        {
            throw new HelperFailure(
                "aax_parse_failed",
                "AAXClean helper could not parse the protected input."
            );
        }
    }

    private static async Task WriteAsync(HelperMessage message)
    {
        await WriteSemaphore.WaitAsync();
        try
        {
            var line = JsonSerializer.Serialize(message, message.GetType(), Protocol.JsonOptions);
            await Console.Out.WriteLineAsync(line);
            await Console.Out.FlushAsync();
        }
        finally
        {
            WriteSemaphore.Release();
        }
    }

    private sealed class HelperFailure : Exception
    {
        internal HelperFailure(string category, string safeMessage)
        {
            Category = category;
            SafeMessage = safeMessage;
        }

        internal string Category { get; }
        internal string SafeMessage { get; }
    }
}
