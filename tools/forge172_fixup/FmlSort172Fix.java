package cpw.mods.fml.relauncher;

import java.util.Arrays;
import java.util.List;
import net.minecraft.launchwrapper.ITweaker;

public final class FmlSort172Fix {
    private FmlSort172Fix() {}

    public static void sortTweakListSafe(List<ITweaker> tweakers) {
        ITweaker[] arr = tweakers.toArray(new ITweaker[tweakers.size()]);
        Arrays.sort(arr, new CoreModManager$2());
        for (int j = 0; j < arr.length; j++) {
            tweakers.set(j, arr[j]);
        }
    }
}
